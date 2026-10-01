use super::{ACTORS, Actor, Effect, Failure, Op, State};
use soroban_sdk::testutils::{
    Address as _, AuthorizedFunction, AuthorizedInvocation, Events,
    IssuerFlags, Ledger, MockAuth, MockAuthInvoke,
};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::{
    Address, Env, Error, IntoVal, Symbol, TryFromVal, TryIntoVal, Val,
    Vec as SVec,
};

pub(super) struct Stellar {
    env: Env,
    token: Address,
    actors: [Address; 4],
    asset_name: soroban_sdk::String,
}
impl Stellar {
    pub fn new(required: bool, height: u32) -> Self {
        let env =
            Env::new_with_config(soroban_sdk::testutils::EnvTestConfig {
                capture_snapshot_at_drop: false,
            });
        env.ledger().with_mut(|l| l.sequence_number = height);
        let actors = ACTORS.map(|_| Address::generate(&env));
        let sac = env.register_stellar_asset_contract_v2(
            actors[Actor::Admin as usize].clone(),
        );
        sac.issuer().set_flag(IssuerFlags::RevocableFlag);
        sac.issuer().set_flag(IssuerFlags::ClawbackEnabledFlag);
        if required {
            sac.issuer().set_flag(IssuerFlags::RequiredFlag);
        }
        let token = sac.address();
        let client = TokenClient::new(&env, &token);
        let asset_name = client.name();
        let expected_name = match sac.asset() {
            soroban_sdk::xdr::Asset::CreditAlphanum4(a) => {
                let code = std::str::from_utf8(&a.asset_code.0)
                    .unwrap()
                    .trim_end_matches('\0');
                // Verify asset-derived metadata, independently of Groundhog's configured strings.
                let issuer: Address =
                    soroban_sdk::xdr::ScAddress::Account(a.issuer)
                        .try_into_val(&env)
                        .unwrap();
                format!("{code}:{}", issuer.to_string())
            }
            other => panic!("unexpected SAC asset: {other:?}"),
        };
        assert_eq!(asset_name.to_string(), expected_name);
        assert_eq!(client.symbol().to_string(), "aaa");
        assert_eq!(client.decimals(), 7);
        Self {
            env,
            token,
            actors,
            asset_name,
        }
    }
    fn addr(&self, a: Actor) -> Address {
        self.actors[a as usize].clone()
    }
    pub fn set_height(&self, height: u32) {
        self.env.ledger().with_mut(|l| l.sequence_number = height);
    }
    pub fn state(&self) -> State {
        let token = TokenClient::new(&self.env, &self.token);
        let asset = StellarAssetClient::new(&self.env, &self.token);
        State {
            balances: ACTORS.map(|a| token.balance(&self.addr(a))),
            allowance: token.allowance(
                &self.addr(Actor::Alice),
                &self.addr(Actor::Spender),
            ),
            authorized: ACTORS.map(|a| asset.authorized(&self.addr(a))),
        }
    }
    pub fn run(
        &mut self,
        op: Op,
        caller: Option<Actor>,
        failure: Option<Failure>,
    ) -> std::vec::Vec<Effect> {
        let env = &self.env;
        let alice = self.addr(Actor::Alice);
        let bob = self.addr(Actor::Bob);
        let spender = self.addr(Actor::Spender);
        let (name, args): (&str, SVec<Val>) = match op {
            Op::Approve(n, d) => {
                ("approve", (&alice, &spender, n, d).into_val(env))
            }
            Op::Transfer(n) => ("transfer", (&alice, &bob, n).into_val(env)),
            Op::Spend(n) => {
                ("transfer_from", (&spender, &alice, &bob, n).into_val(env))
            }
            Op::Burn(n) => ("burn", (&alice, n).into_val(env)),
            Op::BurnFrom(n) => {
                ("burn_from", (&spender, &alice, n).into_val(env))
            }
            Op::Authorize(a, b) => {
                ("set_authorized", (self.addr(a), b).into_val(env))
            }
            Op::Mint(a, n) => ("mint", (self.addr(a), n).into_val(env)),
            Op::Clawback(n) => ("clawback", (&alice, n).into_val(env)),
        };
        let invoke = MockAuthInvoke {
            contract: &self.token,
            fn_name: name,
            args: args.clone(),
            sub_invokes: &[],
        };
        match caller {
            Some(a) => env.mock_auths(&[MockAuth {
                address: &self.addr(a),
                invoke: &invoke,
            }]),
            None => env.mock_auths(&[]),
        }
        let result = env.try_invoke_contract::<(), Error>(
            &self.token,
            &Symbol::new(env, name),
            args.clone(),
        );
        match failure {
            None => {
                assert_eq!(result, Ok(Ok(())));
                assert_eq!(
                    env.auths(),
                    [(
                        self.addr(op.authorizer()),
                        AuthorizedInvocation {
                            function: AuthorizedFunction::Contract((
                                self.token.clone(),
                                Symbol::new(env, name),
                                args
                            )),
                            sub_invocations: vec![],
                        }
                    )],
                    "SAC must require the exact actor, function and arguments"
                );
            }
            Some(f) => {
                let error = result
                    .expect_err("SAC call unexpectedly succeeded")
                    .expect("SAC error conversion");
                let expected = match f {
                    Failure::Negative => Error::from_contract_error(8),
                    Failure::Allowance | Failure::Expiry => {
                        Error::from_contract_error(9)
                    }
                    Failure::Balance => Error::from_contract_error(10),
                    Failure::Overflow => Error::from_contract_error(12),
                    Failure::Frozen => Error::from_contract_error(11),
                    Failure::Auth | Failure::IssuerAuth => {
                        Error::from_type_and_code(
                            soroban_sdk::xdr::ScErrorType::Context,
                            soroban_sdk::xdr::ScErrorCode::InvalidAction,
                        )
                    }
                };
                assert_eq!(error, expected, "{} error", super::ORACLE);
            }
        }
        let all = env.events().all();
        all.events()
            .iter()
            .map(|raw| {
                use soroban_sdk::xdr::{ContractEventBody, ScAddress, ScVal};
                let contract: Address =
                    ScAddress::Contract(raw.contract_id.clone().unwrap())
                        .try_into_val(env)
                        .unwrap();
                let ContractEventBody::V0(body) = &raw.body;
                let topics = SVec::<Val>::try_from_val(
                    env,
                    &ScVal::Vec(Some(body.topics.clone().into())),
                )
                .unwrap();
                let data = Val::try_from_val(env, &body.data).unwrap();
                assert_eq!(contract, self.token);
                let name = Symbol::try_from_val(env, &topics.get(0).unwrap())
                    .unwrap()
                    .to_string();
                let actor = |i| {
                    let addr =
                        Address::try_from_val(env, &topics.get(i).unwrap())
                            .unwrap();
                    ACTORS
                        .into_iter()
                        .find(|a| self.addr(*a) == addr)
                        .expect("known SAC actor")
                };
                let n = || i128::try_from_val(env, &data).unwrap();
                match name.as_str() {
                    "approve" => {
                        assert_eq!(topics.len(), 4);
                        assert_eq!(actor(1), Actor::Alice);
                        assert_eq!(actor(2), Actor::Spender);
                        assert_eq!(
                            soroban_sdk::String::try_from_val(
                                env,
                                &topics.get(3).unwrap()
                            )
                            .unwrap(),
                            self.asset_name
                        );
                        let (n, d) =
                            <(i128, u32)>::try_from_val(env, &data).unwrap();
                        if n == 0 {
                            // SAC retains the supplied deadline in its raw
                            // event; native deletion intentionally does not.
                            assert!(matches!(op, Op::Approve(0, expected) if expected == d));
                            Effect::Revocation
                        } else {
                            Effect::Approval(n, d)
                        }
                    }
                    "transfer" => {
                        assert_eq!(topics.len(), 4);
                        assert_eq!(actor(1), Actor::Alice);
                        assert_eq!(actor(2), Actor::Bob);
                        assert_eq!(
                            soroban_sdk::String::try_from_val(
                                env,
                                &topics.get(3).unwrap()
                            )
                            .unwrap(),
                            self.asset_name
                        );
                        Effect::Transfer(n())
                    }
                    "burn" | "clawback" | "mint" | "set_authorized" => {
                        assert_eq!(topics.len(), 3);
                        assert_eq!(
                            soroban_sdk::String::try_from_val(
                                env,
                                &topics.get(2).unwrap()
                            )
                            .unwrap(),
                            self.asset_name
                        );
                        match name.as_str() {
                            "mint" => Effect::Mint(actor(1), n()),
                            "set_authorized" => Effect::Authorization(
                                actor(1),
                                bool::try_from_val(env, &data).unwrap(),
                            ),
                            "burn" => {
                                assert_eq!(actor(1), Actor::Alice);
                                Effect::Burn(n())
                            }
                            "clawback" => {
                                assert_eq!(actor(1), Actor::Alice);
                                Effect::Clawback(n())
                            }
                            _ => unreachable!(),
                        }
                    }
                    _ => panic!("unknown SAC event: {topics:?} {data:?}"),
                }
            })
            .collect()
    }
}
