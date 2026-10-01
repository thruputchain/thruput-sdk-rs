use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use groundhog_erc20_controls_if::{ClawbackEvent, ERC20ControlsError, Policy};
use groundhog_erc20_if::{AllowanceResetEvent, BurnEvent, ERC20Error};
use groundhog_json_rpc_client::GroundhogApiClient;
use groundhog_testkit::{
    Agent, BlockDriverConfig, DeployOptions, LocalNetworkConfig,
    NetworkConfig, Scenario, WalletInitCall, entrypoint_id,
    rpc_types::{
        events::*,
        hex::Hex0x,
        receipt::{ExecStatus, TransactionReceipt},
    },
    xdr::types::ContractAddress,
    xdrs::{ReadXDROwned, WriteXDR, cxs},
};
use sdk::{Exn, RawExceptionRef};
use sep41_runtime_actor::ActorError;

const ADMIN: usize = 0;
const ALICE: usize = 1;
const BOB: usize = 2;
const SPENDER: usize = 3;
const ACTORS: [&str; 4] = ["admin", "alice", "bob", "spender"];
const ACTOR_INTERFACE: &str = "RuntimeActor";
const VERSION: &str = "0.1.0";

fn required_path(name: &str) -> PathBuf {
    let path = PathBuf::from(
        std::env::var_os(name).unwrap_or_else(|| panic!("set {name}")),
    );
    assert!(
        path.is_file(),
        "{name} must identify a file: {}",
        path.display()
    );
    path
}

struct Runtime {
    scenario: Scenario,
    wallets: [ContractAddress; 4],
    actors: [ContractAddress; 4],
    token: ContractAddress,
}

impl Runtime {
    async fn new() -> Self {
        let mut config = LocalNetworkConfig::default();
        config.hog_devnode_binary =
            Some(required_path("GROUNDHOG_DEVNODE_BINARY"));
        config.block_driver = BlockDriverConfig::manual();
        config.dev_account_count = ACTORS.len();
        let scenario = Scenario::builder("sep41-funded-runtime")
            .network(NetworkConfig::local(config))
            .artifact("issuer", required_path("ISSUER_TOKEN_RV32_ARTIFACT"))
            .artifact("actor", required_path("SEP41_ACTOR_RV32_ARTIFACT"))
            .build()
            .await
            .unwrap();
        let wallets =
            Arc::new(Mutex::new([ContractAddress::from([0; 32]); 4]));
        for selected in 0..4 {
            let wallets = wallets.clone();
            let agents =
                ACTORS.into_iter().enumerate().map(|(index, name)| {
                    let wallets = wallets.clone();
                    Agent::new(name, move |ctx| async move {
                        if index == selected {
                            wallets.lock().unwrap()[index] = ctx.address();
                            let tx = ctx.submit_fund(1_000_000_000).await?;
                            tx.await_pending().await?;
                            ctx.network().produce_block().await?;
                            assert!(matches!(
                                tx.await_receipt().await?.status,
                                ExecStatus::Ok
                            ));
                        }
                        Ok(())
                    })
                });
            scenario.run_agents(agents).await.unwrap();
        }
        let wallets = *wallets.lock().unwrap();
        let token = Self::deploy(
            &scenario,
            ADMIN,
            "issuer",
            "ERC20Admin",
            &(
                wallets[ADMIN],
                "Reference Token",
                "RFT",
                7_u32,
                Policy {
                    auth_required: false,
                    auth_revocable: true,
                    clawback_enabled: true,
                },
            ),
        )
        .await;
        let mut actors = [ContractAddress::from([0; 32]); 4];
        for index in [ALICE, BOB, SPENDER] {
            actors[index] = Self::deploy(
                &scenario,
                index,
                "actor",
                ACTOR_INTERFACE,
                &(token,),
            )
            .await;
        }
        let runtime = Self {
            scenario,
            wallets,
            actors,
            token,
        };
        for index in [ALICE, BOB, SPENDER] {
            assert_eq!(
                runtime
                    .view::<_, ContractAddress>(
                        runtime.actors[index],
                        ACTOR_INTERFACE,
                        "owner",
                        &()
                    )
                    .await,
                wallets[index]
            );
        }
        assert_eq!(
            runtime
                .view::<_, ContractAddress>(token, "ERC20Admin", "admin", &())
                .await,
            wallets[ADMIN]
        );
        runtime
    }

    async fn deploy<A: WriteXDR>(
        scenario: &Scenario,
        selected: usize,
        artifact: &'static str,
        interface: &'static str,
        args: &A,
    ) -> ContractAddress {
        let output = Arc::new(Mutex::new(None));
        let calldata = cxs::encode(args).unwrap();
        let agents = ACTORS.into_iter().enumerate().map(|(index, name)| {
            let output = output.clone();
            let calldata = calldata.clone();
            Agent::new(name, move |mut ctx| async move {
                if index == selected {
                    let deploy = ctx
                        .submit_deploy_artifact(
                            artifact,
                            DeployOptions {
                                amount: 100_000_000,
                                initializer: Some(WalletInitCall {
                                    entrypoint: entrypoint_id(
                                        interface,
                                        VERSION,
                                        "initialize",
                                    ),
                                    gas_limit: 2_000_000,
                                    calldata: &calldata,
                                }),
                            },
                        )
                        .await?;
                    deploy.await_pending().await?;
                    let block = ctx.network().produce_block().await?;
                    let deployed = deploy.await_receipt().await?;
                    assert_eq!(*deployed.receipt.height, block.height);
                    assert!(deployed.receipt.gas_used > 0);
                    *output.lock().unwrap() = Some(deployed.address);
                }
                Ok(())
            })
        });
        scenario.run_agents(agents).await.unwrap();
        output.lock().unwrap().unwrap()
    }

    async fn call<A: WriteXDR>(
        &self,
        selected: usize,
        target: ContractAddress,
        interface: &'static str,
        method: &'static str,
        args: &A,
    ) -> TransactionReceipt {
        let calldata = cxs::encode(args).unwrap();
        let output = Arc::new(Mutex::new(None));
        let agents = ACTORS.into_iter().enumerate().map(|(index, name)| {
            let calldata = calldata.clone();
            let output = output.clone();
            Agent::new(name, move |mut ctx| async move {
                if index == selected {
                    let call = ctx
                        .submit_call(
                            target,
                            entrypoint_id(interface, VERSION, method),
                            &calldata,
                        )
                        .await?;
                    call.await_pending().await?;
                    let block = ctx.network().produce_block().await?;
                    let receipt = call.await_receipt().await?;
                    assert_eq!(*receipt.height, block.height, "{method}");
                    assert!(receipt.gas_used > 0, "{method}");
                    *output.lock().unwrap() = Some(receipt);
                }
                Ok(())
            })
        });
        self.scenario.run_agents(agents).await.unwrap();
        let receipt = output.lock().unwrap().take().unwrap();
        self.check_indexed(&receipt).await;
        receipt
    }

    async fn actor_call<A: WriteXDR>(
        &self,
        selected: usize,
        method: &'static str,
        args: &A,
    ) -> TransactionReceipt {
        self.call(
            selected,
            self.actors[selected],
            ACTOR_INTERFACE,
            method,
            args,
        )
        .await
    }

    async fn fail<A: WriteXDR, E: Exn + ReadXDROwned>(
        &self,
        selected: usize,
        actor: usize,
        method: &'static str,
        args: &A,
        expected: E,
    ) -> TransactionReceipt {
        let receipt = self
            .call(selected, self.actors[actor], ACTOR_INTERFACE, method, args)
            .await;
        assert!(
            matches!(receipt.status, ExecStatus::Reverted { .. }),
            "{receipt:?}"
        );
        assert!(receipt.events.is_empty());
        assert!(
            !receipt
                .abort_data
                .as_ref()
                .expect("wallet abort payload")
                .0
                .is_empty()
        );
        // DefaultWallet wraps child aborts. The signed receipt proves rejection;
        // this matching simulation reveals the child's concrete exception.
        let response = self
            .scenario
            .network()
            .view_call(
                self.actors[actor],
                entrypoint_id(ACTOR_INTERFACE, VERSION, method),
                &cxs::encode(args).unwrap(),
                Some(self.wallets[selected]),
            )
            .await
            .unwrap();
        assert_eq!(response.height, receipt.height);
        assert!(response.events.is_empty());
        let groundhog_testkit::rpc_types::view::RootInvocationOutput::Abort {
            abort_data,
        } = response.output
        else {
            panic!(
                "{method}: expected child abort, got {:?}",
                response.output
            );
        };
        assert_eq!(
            RawExceptionRef::parse(&abort_data.0).unwrap().try_as::<E>(),
            Some(expected),
            "{method}: child exception"
        );
        receipt
    }

    async fn view<A: WriteXDR, R: ReadXDROwned>(
        &self,
        address: ContractAddress,
        interface: &str,
        method: &str,
        args: &A,
    ) -> R {
        let output = self
            .scenario
            .network()
            .view_call(
                address,
                entrypoint_id(interface, VERSION, method),
                &cxs::encode(args).unwrap(),
                None,
            )
            .await
            .unwrap();
        match output.output {
            groundhog_testkit::rpc_types::view::RootInvocationOutput::Return { return_data } => cxs::decode(&return_data.0).unwrap(),
            other => panic!("{method}: {other:?}"),
        }
    }

    async fn height(&self) -> i64 {
        *self.scenario.network().chain_tip().await.unwrap().height
    }

    async fn native_balance(&self, address: ContractAddress) -> u128 {
        self.scenario
            .network()
            .client()
            .unwrap()
            .get_balance(Hex0x(address))
            .await
            .unwrap()
            .raw_balance
            .parse()
            .unwrap()
    }

    async fn state(&self) -> ([i128; 3], i128, i128) {
        let mut balances = [0; 3];
        for (index, actor) in [ALICE, BOB, SPENDER].into_iter().enumerate() {
            balances[index] = self
                .view(
                    self.token,
                    "ERC20",
                    "get_balance_of",
                    &(self.actors[actor],),
                )
                .await;
        }
        let allowance = self
            .view(
                self.token,
                "ERC20",
                "allowance",
                &(self.actors[ALICE], self.actors[SPENDER]),
            )
            .await;
        let supply: i128 =
            self.view(self.token, "ERC20", "total_supply", &()).await;
        assert_eq!(supply, balances.iter().sum());
        (balances, allowance, supply)
    }

    async fn check_indexed(&self, receipt: &TransactionReceipt) {
        assert!(
            receipt
                .events
                .iter()
                .all(|event| event.contract_address.0 == self.token)
        );
        let mut query = EventQuery {
            filters: EventFilterList(
                vec![EventFilter {
                    contract_addresses: ContractAddressList(
                        vec![Hex0x(self.token)].try_into().unwrap(),
                    ),
                    topics: EventTopicFilter {
                        topic0: TopicList::default(),
                        topic1: TopicList::default(),
                        topic2: TopicList::default(),
                    },
                }]
                .try_into()
                .unwrap(),
            ),
            order: IterationOrder::Ascending,
            anchor: BlockAnchor::AtHeight {
                height: receipt.height.clone(),
            },
            cursor: PaginationCursor::default(),
            limit: 1,
        };
        let mut events = vec![];
        loop {
            let page = self
                .scenario
                .network()
                .get_events(query.clone())
                .await
                .unwrap();
            events.extend(
                page.events
                    .into_iter()
                    .filter(|event| event.tx_hash == receipt.tx_hash),
            );
            if page.cursor.0.is_empty() {
                break;
            }
            query.cursor = page.cursor;
        }
        assert_eq!(events.len(), receipt.events.len());
        for (sequence, (indexed, emitted)) in
            events.iter().zip(&receipt.events).enumerate()
        {
            assert_eq!(indexed.block_height, receipt.height);
            assert_eq!(indexed.sequence_number, sequence as u32);
            assert_eq!(
                indexed.event.contract_address,
                emitted.contract_address
            );
            assert_eq!(indexed.event.topics, emitted.topics);
            assert_eq!(indexed.event.data, emitted.data);
        }
    }

    async fn rollback_state(&self) -> (([i128; 3], i128, i128), [u128; 4]) {
        let mut balances = [0; 4];
        for (index, address) in [
            self.token,
            self.actors[ALICE],
            self.actors[BOB],
            self.actors[SPENDER],
        ]
        .into_iter()
        .enumerate()
        {
            balances[index] = self.native_balance(address).await;
        }
        (self.state().await, balances)
    }
}

fn success(receipt: &TransactionReceipt, event_count: usize) {
    assert!(matches!(receipt.status, ExecStatus::Ok), "{receipt:?}");
    assert_eq!(receipt.events.len(), event_count);
}

#[tokio::test]
async fn signed_funded_sep41_calls() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let runtime = Runtime::new().await;
        let alice = runtime.actors[ALICE];
        let bob = runtime.actors[BOB];
        let spender = runtime.actors[SPENDER];
        assert_eq!(runtime.state().await, ([0, 0, 0], 0, 0));
        assert!(runtime.native_balance(runtime.token).await < 100_000_000);
        let before_mint = runtime.native_balance(runtime.token).await;
        success(
            &runtime
                .call(
                    ADMIN,
                    runtime.token,
                    "ERC20",
                    "mint",
                    &(alice, 100_i128),
                )
                .await,
            1,
        );
        assert!(
            runtime.native_balance(runtime.token).await < before_mint,
            "holder storage reserves HOG"
        );
        assert_eq!(runtime.state().await, ([100, 0, 0], 0, 100));

        // The same Bob signer can execute his own actor's guarded method.
        success(
            &runtime
                .actor_call(BOB, "approve", &(spender, 0_i128, 0_i64))
                .await,
            1,
        );

        let before = runtime.rollback_state().await;
        runtime
            .fail(
                BOB,
                ALICE,
                "approve",
                &(spender, 50_i128, runtime.height().await + 20),
                ActorError::Unauthorized,
            )
            .await;
        assert_eq!(runtime.rollback_state().await, before);
        runtime
            .fail(
                ALICE,
                ALICE,
                "initialize",
                &(runtime.token,),
                ActorError::AlreadyInitialized,
            )
            .await;
        assert_eq!(
            runtime
                .view::<_, ContractAddress>(
                    alice,
                    ACTOR_INTERFACE,
                    "owner",
                    &()
                )
                .await,
            runtime.wallets[ALICE]
        );

        let until = runtime.height().await + 20;
        success(
            &runtime
                .actor_call(ALICE, "approve", &(spender, 50_i128, until))
                .await,
            2,
        );
        success(
            &runtime
                .actor_call(SPENDER, "spend", &(alice, bob, 12_i128))
                .await,
            1,
        );
        assert_eq!(runtime.state().await, ([88, 12, 0], 38, 100));
        success(
            &runtime
                .actor_call(ALICE, "approve", &(spender, 20_i128, until))
                .await,
            2,
        );
        assert_eq!(runtime.state().await, ([88, 12, 0], 20, 100));

        success(
            &runtime
                .call(
                    ADMIN,
                    runtime.token,
                    "ERC20Authorization",
                    "set_authorized",
                    &(bob, false),
                )
                .await,
            1,
        );
        assert!(
            !runtime
                .view::<_, bool>(
                    runtime.token,
                    "ERC20Authorization",
                    "authorized",
                    &(bob,)
                )
                .await
        );
        let before = runtime.rollback_state().await;
        runtime
            .fail(
                SPENDER,
                SPENDER,
                "spend",
                &(alice, bob, 3_i128),
                ERC20ControlsError::HolderNotAuthorized,
            )
            .await;
        assert_eq!(
            runtime.rollback_state().await,
            before,
            "recipient freeze restores prior allowance debit"
        );
        success(
            &runtime
                .call(
                    ADMIN,
                    runtime.token,
                    "ERC20Authorization",
                    "set_authorized",
                    &(bob, true),
                )
                .await,
            1,
        );
        assert!(
            runtime
                .view::<_, bool>(
                    runtime.token,
                    "ERC20Authorization",
                    "authorized",
                    &(bob,)
                )
                .await
        );
        success(
            &runtime
                .actor_call(SPENDER, "spend", &(alice, bob, 3_i128))
                .await,
            1,
        );
        assert_eq!(runtime.state().await, ([85, 15, 0], 17, 100));

        let before = runtime.rollback_state().await;
        runtime
            .fail(
                ALICE,
                ALICE,
                "transfer_then_abort",
                &(spender, 5_i128),
                ActorError::DeliberateAbort,
            )
            .await;
        assert_eq!(
            runtime.rollback_state().await,
            before,
            "parent abort restores child balances, collateral and logs"
        );

        let until = runtime.height().await + 2;
        success(
            &runtime
                .actor_call(ALICE, "approve", &(spender, 10_i128, until))
                .await,
            2,
        );
        let receipt = runtime
            .actor_call(SPENDER, "spend", &(alice, bob, 4_i128))
            .await;
        success(&receipt, 1);
        assert_eq!(*receipt.height, until, "expiry is inclusive");
        // Views simulate the next execution block, so the remaining six tokens
        // already read as expired after the inclusive-boundary spend commits.
        assert_eq!(runtime.state().await, ([81, 19, 0], 0, 100));
        let before = runtime.rollback_state().await;
        let receipt = runtime
            .fail(
                SPENDER,
                SPENDER,
                "spend",
                &(alice, bob, 1_i128),
                ERC20Error::TooLarge,
            )
            .await;
        assert_eq!(*receipt.height, until + 1, "first expired block");
        assert_eq!(runtime.rollback_state().await, before);

        let until = runtime.height().await + 10;
        let receipt = runtime
            .actor_call(ALICE, "approve", &(spender, 9_i128, until))
            .await;
        success(&receipt, 2);
        assert_eq!(
            &receipt.events[0].topics[0].0,
            AllowanceResetEvent::SIGNATURE
        );
        assert_eq!(
            cxs::decode::<(i128, i64)>(&receipt.events[0].data.0).unwrap(),
            (6, until)
        );
        success(
            &runtime
                .actor_call(ALICE, "approve", &(spender, 0_i128, 0_i64))
                .await,
            1,
        );
        assert_eq!(runtime.state().await, ([81, 19, 0], 0, 100));
        let before = runtime.rollback_state().await;
        runtime
            .fail(
                SPENDER,
                SPENDER,
                "spend",
                &(alice, bob, 1_i128),
                ERC20Error::TooLarge,
            )
            .await;
        assert_eq!(runtime.rollback_state().await, before);

        let receipt = runtime
            .call(
                ADMIN,
                runtime.token,
                "ERC20Clawback",
                "clawback",
                &(alice, 7_i128),
            )
            .await;
        success(&receipt, 2);
        for (event, signature) in receipt
            .events
            .iter()
            .zip([BurnEvent::SIGNATURE, ClawbackEvent::SIGNATURE])
        {
            assert_eq!(event.topics.len(), 2);
            assert_eq!(&event.topics[0].0, signature);
            assert_eq!(event.topics[1].0.as_slice(), alice.as_ref());
            assert_eq!(cxs::decode::<i128>(&event.data.0).unwrap(), 7);
        }
        assert_eq!(runtime.state().await, ([74, 19, 0], 0, 93));
        runtime.scenario.stop().await.unwrap();
    })
    .await
    .expect("funded runtime finishes within two minutes");
}
