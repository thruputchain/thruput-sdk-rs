//! `CodeHash<C>` and `DeployNonce` stand in for a `SHA256Hash` everywhere one
//! is stored or passed, encode identically, and deploy to an `Address<C>`.

#![allow(unexpected_cfgs)]

use thruput_contract_spec::ContractTypeRefSpec;
use thruput_sdk::storage::{Map, Value};
use thruput_sdk::xdr::cxs;
use thruput_sdk::*;

pub struct Drop;

#[contract]
impl Drop {
    pub fn label() -> u32 {
        42
    }
}

#[contract(version = "1.0.0")]
pub trait Metadata {
    fn name_len() -> u32;
}

#[contract]
impl Metadata for Drop {
    fn name_len() -> u32 {
        5
    }
}

/// Carries a typed code hash, with the derives a `SHA256Hash` field allows.
#[contract_type]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub drop_code: CodeHash<DropClient>,
    pub salt: DeployNonce,
    pub fee: u32,
}

/// The same shape with untyped hashes.
#[contract_type]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UntypedConfig {
    pub drop_code: SHA256Hash,
    pub salt: SHA256Hash,
    pub fee: u32,
}

#[derive(Event, Clone, Debug)]
pub struct Created {
    #[topic]
    pub code: CodeHash<DropClient>,
    #[topic]
    pub nonce: DeployNonce,
    pub drop: Address<DropClient>,
}

const DOMAIN: u32 = 0x4452_4f50;

#[thruput_sdk::storage]
pub struct Factory {
    config: Value<Config>,
    code: Value<CodeHash<DropClient>>,
    both: Value<CodeHash<(DropClient, MetadataClient)>>,
    created: Map<DeployNonce, Value<Address<DropClient>>>,
    fees: Map<CodeHash<DropClient>, Value<u32>>,
}

impl Factory {
    /// A nonce in the namespace of `creator`.
    fn nonce(creator: &ContractAddress, salt: &DeployNonce) -> DeployNonce {
        DeployNonce::derive(&(DOMAIN, creator, salt))
    }
}

#[contract]
impl Factory {
    pub fn set_config(&mut self, config: Config) {
        self.fees.at_mut(&config.drop_code).set(&config.fee);
        self.code.set(&config.drop_code);
        self.config.set(&config);
    }

    pub fn set_both(&mut self, both: &CodeHash<(DropClient, MetadataClient)>) {
        self.both.set(both);
    }

    pub fn config(&self) -> Config {
        self.config.get()
    }

    pub fn code(&self) -> CodeHash<DropClient> {
        self.code.get()
    }

    /// Reads the entry through the untyped hash: a typed key is the same
    /// storage key.
    pub fn fee(&self, code: &SHA256Hash) -> u32 {
        self.fees.at(CodeHash::from_ref(code)).get()
    }

    pub fn count(&self, salts: &[DeployNonce]) -> u32 {
        salts.len() as u32
    }

    pub fn predict(
        &self,
        creator: &ContractAddress,
        salt: &DeployNonce,
    ) -> Address<DropClient> {
        self.code.get().deploy_address(&Self::nonce(creator, salt))
    }

    /// Deploys in the caller's namespace and calls the new contract through
    /// the client the code hash names.
    pub fn create(&mut self, salt: &DeployNonce) -> Address<DropClient> {
        let nonce = Self::nonce(&get_caller(), salt);
        let drop = self.code.get().deploy(&nonce);
        assert_eq!(drop.client().label(), 42);
        self.created.at_mut(salt).set(&drop);
        drop
    }

    pub fn created(&self, salt: &DeployNonce) -> Address<DropClient> {
        self.created.at(salt).get()
    }

    /// Calls both interfaces of one deployment.
    pub fn create_both(&mut self, salt: &DeployNonce) -> u32 {
        let both = self.both.get().deploy(salt);
        DropClient::at(&both).label() + MetadataClient::at(&both).name_len()
    }
}

const FACTORY: ContractAddress = ContractAddress::new([0x43; 32]);
const CALLER: ContractAddress = ContractAddress::new([0x41; 32]);
const OTHER_CALLER: ContractAddress = ContractAddress::new([0x44; 32]);
const HASH: SHA256Hash = SHA256Hash::new([0x11; 32]);
const SALT: DeployNonce = DeployNonce::new(SHA256Hash::new([0x22; 32]));

/// `Drop` implements both of its interfaces in one contract.
fn drop_mock(contract: &mut MockContract) {
    Drop::__mock(contract);
    <Drop as Metadata>::__mock(contract);
}

/// Runs `body` as `caller`, with the factory deployed and the drop code
/// registered but not deployed.
fn run_as(
    caller: ContractAddress,
    body: impl FnOnce(CodeHash<DropClient>) + std::panic::UnwindSafe + 'static,
) {
    let env = TestEnvironment::no_cost();
    let code = CodeHash::<DropClient>::of_mock(&env.register_mock(drop_mock));
    env.register_mock(Factory::__mock)
        .deploy_contract_to(&FACTORY);
    env.transact(|tx| tx.call(&caller).run(move || body(code)))
        .assert_success();
}

fn run(
    body: impl FnOnce(CodeHash<DropClient>) + std::panic::UnwindSafe + 'static,
) {
    run_as(CALLER, body);
}

#[test]
fn encode_as_a_hash() {
    let code = CodeHash::<DropClient>::new(HASH);
    assert_eq!(cxs::encode(&code).unwrap(), cxs::encode(&HASH).unwrap());
    assert_eq!(
        cxs::encode(&SALT).unwrap(),
        cxs::encode(SALT.as_hash()).unwrap()
    );

    let config = Config {
        drop_code: code,
        salt: SALT,
        fee: 7,
    };
    let untyped = UntypedConfig {
        drop_code: HASH,
        salt: SALT.into_hash(),
        fee: 7,
    };
    let bytes = cxs::encode(&config).unwrap();
    assert_eq!(bytes, cxs::encode(&untyped).unwrap());
    assert_eq!(cxs::decode::<Config>(&bytes).unwrap(), config);

    let max = <SHA256Hash as cxs::ConstMaxSize>::MAX_SIZE;
    assert_eq!(<CodeHash<DropClient> as cxs::ConstMaxSize>::MAX_SIZE, max);
    assert_eq!(<DeployNonce as cxs::ConstMaxSize>::MAX_SIZE, max);
    assert_eq!(
        core::mem::size_of::<CodeHash<(DropClient, MetadataClient)>>(),
        core::mem::size_of::<SHA256Hash>()
    );
}

#[test]
fn have_the_spec_type_of_a_hash() {
    let hash = <SHA256Hash as ContractTypeRefSpec>::INFO.ty;
    assert_eq!(<CodeHash<DropClient> as ContractTypeRefSpec>::INFO.ty, hash);
    assert_eq!(<DeployNonce as ContractTypeRefSpec>::INFO.ty, hash);
}

#[test]
fn have_the_topic_of_a_hash() {
    let code = CodeHash::<DropClient>::new(HASH);
    assert_eq!(
        Topic::from(code.into_topic()),
        Topic::from(HASH.into_topic()),
    );
    assert_eq!(
        Topic::from(SALT.into_topic()),
        Topic::from(SALT.as_hash().into_topic()),
    );
}

#[test]
fn convert_and_compare_with_a_hash() {
    let code = CodeHash::<DropClient>::new(HASH);
    assert_eq!(code, HASH);
    assert_eq!(HASH, code);
    assert_eq!(code.as_hash(), &HASH);
    assert_eq!(SHA256Hash::from(code), HASH);
    assert_eq!(CodeHash::<DropClient>::from_ref(&HASH), &code);
    assert_eq!(code.cast::<FactoryClient>().into_hash(), HASH);
    assert_eq!(format!("{code:?}"), format!("{HASH:?}"));
    assert_eq!(CodeHash::<DropClient>::default(), SHA256Hash::default());

    let nonce = DeployNonce::new(HASH);
    assert_eq!(nonce, HASH);
    assert_eq!(HASH, nonce);
    assert_eq!(SHA256Hash::from(nonce), HASH);
    assert_eq!(DeployNonce::from_ref(&HASH), &nonce);
    assert_eq!(format!("{nonce:?}"), format!("{HASH:?}"));
    assert_eq!(DeployNonce::default(), SHA256Hash::default());
}

#[test]
fn derived_nonce_is_the_hash_of_the_preimage() {
    run(|_| {
        let preimage = (DOMAIN, &CALLER, &SALT);
        assert_eq!(DeployNonce::derive(&preimage), preimage.hash());
        // The typed salt hashes as the untyped one does.
        assert_eq!(
            DeployNonce::derive(&preimage),
            (DOMAIN, &CALLER, SALT.as_hash()).hash()
        );
        assert_ne!(
            DeployNonce::derive(&preimage),
            DeployNonce::derive(&(DOMAIN, &OTHER_CALLER, &SALT))
        );
    });
}

#[test]
fn deploys_to_the_predicted_typed_address() {
    run(|code| {
        let predicted: Address<DropClient> = code.deploy_address(&SALT);
        assert_eq!(code.deploy_address_for(&CALLER, &SALT), predicted);
        assert_ne!(code.deploy_address_for(&FACTORY, &SALT), predicted);

        // The typed and untyped forms agree.
        assert_eq!(
            predicted,
            get_deploy_address(SALT.as_hash(), code.as_hash())
        );
        assert_eq!(
            predicted,
            deploy_address(&CALLER, SALT.as_hash(), code.as_hash())
        );

        let drop = code.deploy(&SALT);
        assert_eq!(drop, predicted);
        assert_eq!(drop.client().label(), 42);

        // A second nonce is a second contract.
        let other = code.deploy(&DeployNonce::derive(&(DOMAIN, 1u32)));
        assert_ne!(other, drop);
        assert_eq!(other.client().label(), 42);
    });
}

#[test]
fn stored_code_hash_deploys_and_gives_the_client() {
    run(|code| {
        let factory = FactoryClient::new(&FACTORY);
        let config = Config {
            drop_code: code,
            salt: SALT,
            fee: 7,
        };
        factory.set_config(config.clone());

        assert_eq!(factory.config(), config);
        assert_eq!(factory.code(), code);
        assert_eq!(factory.fee(code.as_hash()), 7);
        assert_eq!(factory.count(&[SALT, SALT]), 2);

        let predicted = factory.predict(&CALLER, &SALT);
        let drop = factory.create(&SALT);
        assert_eq!(drop, predicted);
        assert_eq!(factory.created(&SALT), drop);
        assert_eq!(drop.client().label(), 42);

        // The factory is the deployer, and the caller is in the nonce.
        let nonce = DeployNonce::derive(&(DOMAIN, &CALLER, &SALT));
        assert_eq!(drop, code.deploy_address_for(&FACTORY, &nonce));
        assert_ne!(drop, factory.predict(&OTHER_CALLER, &SALT));
    });
}

#[test]
fn each_caller_has_its_own_addresses() {
    run_as(OTHER_CALLER, |code| {
        let factory = FactoryClient::new(&FACTORY);
        factory.set_config(Config {
            drop_code: code,
            salt: SALT,
            fee: 0,
        });
        let drop = factory.create(&SALT);
        assert_eq!(drop, factory.predict(&OTHER_CALLER, &SALT));
        assert_ne!(drop, factory.predict(&CALLER, &SALT));
    });
}

#[test]
fn several_clients_share_one_code_hash() {
    run(|code| {
        let both = code.cast::<(DropClient, MetadataClient)>();
        let factory = FactoryClient::new(&FACTORY);
        factory.set_both(&both);
        assert_eq!(factory.create_both(&SALT), 47);

        let drop = both.deploy(&SALT);
        assert_eq!(DropClient::at(&drop).label(), 42);
        assert_eq!(MetadataClient::at(&drop).name_len(), 5);
        let narrowed: Address<MetadataClient> = drop.narrow();
        assert_eq!(narrowed.client().name_len(), 5);
    });
}
