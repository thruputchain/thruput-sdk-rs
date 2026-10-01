use super::{ACTORS, Actor, Effect, Failure, Op, State};
use groundhog_erc20_controls_if::*;
use groundhog_erc20_if::*;
use groundhog_sep41::{SEP41Error, SEP41View};
use sdk::*;

const TOKEN: ContractAddress = ContractAddress::new([0x30; 32]);
fn address(actor: Actor) -> ContractAddress {
    ContractAddress::new([0x31 + actor as u8; 32])
}
fn signer(actor: Actor) -> ContractAddress {
    ContractAddress::new([0x41 + actor as u8; 32])
}

#[contract_exception]
pub enum WalletError {
    Unauthorized = 1,
}

#[sdk::storage]
pub struct Wallet;

// A host fixture, not a signed wallet: the test host supplies the external
// caller. Token calls run in this authenticated wallet's nested frame.
#[contract]
impl Wallet {
    pub fn execute(
        &self,
        owner: u32,
        operation: u32,
        actor: u32,
        amount: i128,
        deadline: u32,
        authorized: bool,
    ) {
        let owner = ACTORS[owner as usize];
        ensure(
            get_current_address() == address(owner),
            WalletError::Unauthorized,
        );
        ensure(get_caller() == signer(owner), WalletError::Unauthorized);
        let token = SEP41View::new(&TOKEN);
        let alice = address(Actor::Alice);
        let bob = address(Actor::Bob);
        let spender = address(Actor::Spender);
        match operation {
            0 => token.approve(
                &spender,
                amount,
                Some(i64::from(deadline).into()),
            ),
            1 => token.transfer(&bob, amount),
            2 => token.transfer_from(&alice, &bob, amount),
            3 => token.burn(amount),
            4 => token.burn_from(&alice, amount),
            5 => ERC20AuthorizationClient::new(&TOKEN)
                .set_authorized(&address(ACTORS[actor as usize]), authorized),
            6 => ERC20Client::new(&TOKEN)
                .mint(&address(ACTORS[actor as usize]), amount),
            7 => ERC20ClawbackClient::new(&TOKEN).clawback(&alice, amount),
            _ => panic!("unknown wallet operation"),
        }
    }
}

pub(super) struct Groundhog {
    env: TestEnvironmentHandle,
}
impl Groundhog {
    pub fn new(required: bool) -> Self {
        let path = std::env::var_os("ISSUER_TOKEN_RV32_ARTIFACT").expect(
            "ISSUER_TOKEN_RV32_ARTIFACT is required; build erc20_issuer first",
        );
        let contract = sdk::types::Contract::kRv32 {
            rv32_contract: sdk::types::contract::ContractBlob {
                data: sdk::xdr::BytesM(
                    std::fs::read(path)
                        .expect("read issuer token RV32 artifact"),
                ),
            },
        };
        let env = TestEnvironment::no_cost();
        env.setup_create_contract(&contract);
        env.setup_create_deployment(&TOKEN, &contract);
        let wallet = sdk::register_mock_package(&env, env!("CARGO_PKG_NAME"))
            .as_contract();
        env.setup_create_contract(&wallet);
        for actor in ACTORS {
            env.setup_create_deployment(&address(actor), &wallet);
        }
        let admin = address(Actor::Admin);
        let result = env.transact(|tx| {
            tx.call(&admin).run(|| {
                ERC20AdminClient::new(&TOKEN).initialize(
                    &admin,
                    "Reference Token".try_into().unwrap(),
                    "TEST".try_into().unwrap(),
                    7,
                    Policy {
                        auth_required: required,
                        auth_revocable: true,
                        clawback_enabled: true,
                    },
                );
                let token = SEP41View::new(&TOKEN);
                assert_eq!(token.name().as_str(), "Reference Token");
                assert_eq!(token.symbol().as_str(), "TEST");
                assert_eq!(token.decimals(), 7);
            })
        });
        result.assert_success();
        result.assert_no_events();
        env.advance_block();
        Self { env }
    }
    pub fn height(&self) -> u32 {
        let r = self.env.transact(|tx| {
            tx.call(&signer(Actor::Bob))
                .run(sdk::block::get_block_height)
        });
        r.assert_success();
        r.assert_return_as::<BlockHeight>()
            .into_inner()
            .try_into()
            .unwrap()
    }
    pub fn advance(&self) {
        self.env.advance_block();
    }
    pub fn state(&self) -> State {
        let r = self.env.transact(|tx| {
            tx.call(&signer(Actor::Bob)).run(|| {
                let token = SEP41View::new(&TOKEN);
                let auth = ERC20AuthorizationClient::new(&TOKEN);
                (
                    ACTORS.map(|a| token.balance(&address(a))),
                    token.allowance(
                        &address(Actor::Alice),
                        &address(Actor::Spender),
                    ),
                    ACTORS.map(|a| auth.authorized(&address(a))),
                    ERC20Client::new(&TOKEN).total_supply(),
                )
            })
        });
        r.assert_success();
        r.assert_no_events();
        let (balances, allowance, authorized, supply): (
            [i128; 4],
            i128,
            [bool; 4],
            i128,
        ) = r.assert_return_as();
        assert_eq!(supply, balances.iter().sum::<i128>());
        State {
            balances,
            allowance,
            authorized,
        }
    }
    pub fn run(
        &self,
        op: Op,
        caller: Option<Actor>,
        failure: Option<Failure>,
        stored_allowance: i128,
    ) -> Vec<Effect> {
        let wallet_owner = if failure == Some(Failure::IssuerAuth) {
            assert_eq!(op.authorizer(), Actor::Admin);
            caller.expect("issuer test must authenticate a wallet")
        } else {
            op.authorizer()
        };
        let r = self.env.transact(|tx| {
            tx.call(
                &caller
                    .map(signer)
                    .unwrap_or(ContractAddress::new([0x77; 32])),
            )
            .run(|| {
                let (code, actor, amount, deadline, authorized) = match op {
                    Op::Approve(n, d) => (0, 0, n, d, false),
                    Op::Transfer(n) => (1, 0, n, 0, false),
                    Op::Spend(n) => (2, 0, n, 0, false),
                    Op::Burn(n) => (3, 0, n, 0, false),
                    Op::BurnFrom(n) => (4, 0, n, 0, false),
                    Op::Authorize(a, b) => (5, a as u32, 0, 0, b),
                    Op::Mint(a, n) => (6, a as u32, n, 0, false),
                    Op::Clawback(n) => (7, 0, n, 0, false),
                };
                WalletClient::new(&address(wallet_owner)).execute(
                    wallet_owner as u32,
                    code,
                    actor,
                    amount,
                    deadline,
                    authorized,
                );
            })
        });
        match failure {
            None => r.assert_success(),
            Some(Failure::Auth) => assert_eq!(
                r.assert_abort_as::<WalletError>(),
                WalletError::Unauthorized
            ),
            Some(Failure::IssuerAuth) => match op {
                Op::Authorize(..) => assert_eq!(
                    r.assert_abort_as::<ERC20ControlsError>(),
                    ERC20ControlsError::Unauthorized
                ),
                Op::Mint(..) | Op::Clawback(..) => assert_eq!(
                    r.assert_abort_as::<ERC20Error>(),
                    ERC20Error::MustBeContractOwner
                ),
                _ => panic!(
                    "issuer authorization test requires an issuer operation"
                ),
            },
            Some(Failure::Negative) => assert_eq!(
                r.assert_abort_as::<ERC20Error>(),
                ERC20Error::NegativeAmount
            ),
            Some(Failure::Expiry) => assert_eq!(
                r.assert_abort_as::<SEP41Error>(),
                SEP41Error::InvalidExpiration
            ),
            Some(Failure::Frozen) => assert_eq!(
                r.assert_abort_as::<ERC20ControlsError>(),
                ERC20ControlsError::HolderNotAuthorized
            ),
            Some(
                Failure::Allowance | Failure::Balance | Failure::Overflow,
            ) => assert_eq!(
                r.assert_abort_as::<ERC20Error>(),
                ERC20Error::TooLarge
            ),
        }
        if failure.is_some() {
            r.assert_no_events();
        }
        let events = r
            .events()
            .iter()
            .map(|raw| {
                assert_eq!(
                    raw.contract_address, TOKEN,
                    "only the token emits these events"
                );
                let e = EmittedEvent::from(raw);
                let sig = e.topic(0).unwrap();
                let actor = |i| {
                    ACTORS
                        .into_iter()
                        .find(|a| e.topic(i).unwrap() == address(*a).as_ref())
                        .expect("known Groundhog actor")
                };
                if sig == AllowanceResetEvent::SIGNATURE
                    || sig == AllowanceDeleteEvent::SIGNATURE
                    || sig == AllowanceChangeEvent::SIGNATURE
                {
                    assert_eq!(e.num_topics(), 3);
                    assert_eq!(actor(1), Actor::Alice);
                    assert_eq!(actor(2), Actor::Spender);
                    if sig == AllowanceResetEvent::SIGNATURE {
                        let event = AllowanceResetEvent::try_from(e).unwrap();
                        NativeEffect::Reset {
                            removed: event.removed,
                            until: event
                                .until
                                .into_inner()
                                .try_into()
                                .unwrap(),
                        }
                    } else if sig == AllowanceDeleteEvent::SIGNATURE {
                        NativeEffect::Delete {
                            removed: AllowanceDeleteEvent::try_from(e)
                                .unwrap()
                                .removed,
                        }
                    } else {
                        NativeEffect::Change(
                            AllowanceChangeEvent::try_from(e)
                                .unwrap()
                                .difference,
                        )
                    }
                } else if sig == TransferEvent::SIGNATURE {
                    assert_eq!(e.num_topics(), 3);
                    assert_eq!(actor(1), Actor::Alice);
                    assert_eq!(actor(2), Actor::Bob);
                    NativeEffect::Movement(Effect::Transfer(
                        sdk::internal::decode_xdr(e.payload().unwrap())
                            .unwrap(),
                    ))
                } else if sig == BurnEvent::SIGNATURE
                    || sig == ClawbackEvent::SIGNATURE
                {
                    assert_eq!(e.num_topics(), 2);
                    assert_eq!(actor(1), Actor::Alice);
                    let n = sdk::internal::decode_xdr(e.payload().unwrap())
                        .unwrap();
                    NativeEffect::Movement(if sig == BurnEvent::SIGNATURE {
                        Effect::Burn(n)
                    } else {
                        Effect::Clawback(n)
                    })
                } else if sig == MintEvent::SIGNATURE {
                    assert_eq!(e.num_topics(), 2);
                    NativeEffect::Movement(Effect::Mint(
                        actor(1),
                        sdk::internal::decode_xdr(e.payload().unwrap())
                            .unwrap(),
                    ))
                } else if sig == SetAuthorizedEvent::SIGNATURE {
                    assert_eq!(e.num_topics(), 2);
                    NativeEffect::Movement(Effect::Authorization(
                        actor(1),
                        sdk::internal::decode_xdr(e.payload().unwrap())
                            .unwrap(),
                    ))
                } else {
                    panic!("unknown Groundhog event: {raw:?}");
                }
            })
            .collect();
        let mut effects = normalize(events, stored_allowance);
        if failure.is_none()
            && let Op::Clawback(n) = op
        {
            assert_eq!(
                effects,
                [Effect::Burn(n), Effect::Clawback(n)],
                "Groundhog clawback emits both in order"
            );
            effects = vec![Effect::Clawback(n)];
        }
        effects
    }
}

#[derive(Debug)]
enum NativeEffect {
    Reset { removed: i128, until: u32 },
    Delete { removed: i128 },
    Change(i128),
    Movement(Effect),
}

// Finite approvals are recoverable from this token's reset/change pair. A
// zero delete has no deadline; compare revocation without inventing one.
fn normalize(
    events: Vec<NativeEffect>,
    stored_allowance: i128,
) -> Vec<Effect> {
    match events.as_slice() {
        [
            NativeEffect::Reset { removed, until },
            NativeEffect::Change(amount),
        ] => {
            assert_eq!(
                *removed, stored_allowance,
                "reset reports the stored grant removed"
            );
            assert!(*amount > 0, "replacement must be a positive grant");
            vec![Effect::Approval(*amount, *until)]
        }
        [NativeEffect::Delete { removed }] => {
            assert_eq!(
                *removed, stored_allowance,
                "delete reports the stored grant removed"
            );
            vec![Effect::Revocation]
        }
        _ => events
            .into_iter()
            .map(|e| match e {
                NativeEffect::Movement(effect) => effect,
                _ => panic!("unexpected allowance event sequence"),
            })
            .collect(),
    }
}

#[test]
#[should_panic(expected = "unexpected allowance event sequence")]
fn normalizer_rejects_an_unpaired_change() {
    normalize(vec![NativeEffect::Change(10)], 0);
}
#[test]
#[should_panic(expected = "reset reports the stored grant removed")]
fn normalizer_checks_removed_amount_even_after_expiry() {
    normalize(
        vec![
            NativeEffect::Reset {
                removed: 0,
                until: 20,
            },
            NativeEffect::Change(10),
        ],
        7,
    );
}
#[test]
#[should_panic(expected = "unexpected allowance event sequence")]
fn normalizer_rejects_an_extra_event_between_reset_and_change() {
    normalize(
        vec![
            NativeEffect::Reset {
                removed: 0,
                until: 20,
            },
            NativeEffect::Movement(Effect::Transfer(1)),
            NativeEffect::Change(10),
        ],
        0,
    );
}
