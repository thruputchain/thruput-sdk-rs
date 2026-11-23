//! `Address<C>` stands in for a `ContractAddress` everywhere one is stored
//! or passed, encodes identically, and gives a client for each interface it
//! names.

#![allow(unexpected_cfgs)]

use thruput_contract_spec::ContractTypeRefSpec;
use thruput_sdk::storage::{Map, Value};
use thruput_sdk::xdr::cxs;
use thruput_sdk::*;

pub struct Token;

#[contract]
impl Token {
    pub fn label() -> u32 {
        42
    }
}

#[contract(version = "1.0.0")]
pub trait Metadata {
    fn name_len() -> u32;
}

#[contract]
impl Metadata for Token {
    fn name_len() -> u32 {
        5
    }
}

/// Carries a typed address, with the derives a `ContractAddress` field
/// allows.
#[contract_type]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Terms {
    pub token: Address<TokenClient>,
    pub amount: u32,
}

/// The same shape with an untyped address.
#[contract_type]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UntypedTerms {
    pub token: ContractAddress,
    pub amount: u32,
}

#[derive(Event, Clone, Debug)]
pub struct Listed {
    #[topic]
    pub token: Address<TokenClient>,
    pub amount: u32,
}

#[thruput_sdk::storage]
pub struct Holder {
    terms: Value<Terms>,
    token: Value<Address<TokenClient>>,
    listed: Map<Address<TokenClient>, Value<u32>>,
    both: Value<Address<(TokenClient, MetadataClient)>>,
}

#[contract]
impl Holder {
    pub fn set_terms(&mut self, terms: Terms) {
        self.listed.at_mut(&terms.token).set(&terms.amount);
        self.terms.set(&terms);
    }

    pub fn set_token(&mut self, token: &Address<TokenClient>) {
        self.token.set(token);
    }

    pub fn terms(&self) -> Terms {
        self.terms.get()
    }

    pub fn token(&self) -> Address<TokenClient> {
        self.token.get()
    }

    /// Reads the entry through the untyped address: a typed key is the same
    /// storage key.
    pub fn listed(&self, token: &ContractAddress) -> u32 {
        self.listed.at(Address::from_ref(token)).get()
    }

    pub fn count(&self, tokens: &[Address<TokenClient>]) -> u32 {
        tokens.len() as u32
    }

    pub fn set_both(&mut self, both: &Address<(TokenClient, MetadataClient)>) {
        self.both.set(both);
    }

    /// Calls both interfaces of one stored address.
    pub fn label_plus_name_len(&self) -> u32 {
        let both = self.both.get();
        TokenClient::at(&both).label() + MetadataClient::at(&both).name_len()
    }

    pub fn label_from_terms(&self) -> u32 {
        self.terms.get().token.client().label()
    }

    pub fn label_from_token(&self) -> u32 {
        self.token.get().client().label()
    }
}

const TOKEN: ContractAddress = ContractAddress::new([0x42; 32]);
const HOLDER: ContractAddress = ContractAddress::new([0x43; 32]);
const CALLER: ContractAddress = ContractAddress::new([0x41; 32]);

/// `Token` implements both of its interfaces at one address.
fn token_mock(contract: &mut MockContract) {
    Token::__mock(contract);
    <Token as Metadata>::__mock(contract);
}

fn run(body: impl FnOnce() + std::panic::UnwindSafe + 'static) {
    let env = TestEnvironment::no_cost();
    env.register_mock(token_mock).deploy_contract_to(&TOKEN);
    env.register_mock(Holder::__mock)
        .deploy_contract_to(&HOLDER);
    env.transact(|tx| tx.call(&CALLER).run(body))
        .assert_success();
}

#[test]
fn encodes_as_a_contract_address() {
    let typed: Address<TokenClient> = TOKEN.into();
    assert_eq!(cxs::encode(&typed).unwrap(), cxs::encode(&TOKEN).unwrap());

    let terms = Terms {
        token: typed,
        amount: 7,
    };
    let untyped = UntypedTerms {
        token: TOKEN,
        amount: 7,
    };
    let bytes = cxs::encode(&terms).unwrap();
    assert_eq!(bytes, cxs::encode(&untyped).unwrap());
    assert_eq!(cxs::decode::<Terms>(&bytes).unwrap(), terms);

    assert_eq!(
        <Address<TokenClient> as cxs::ConstMaxSize>::MAX_SIZE,
        <ContractAddress as cxs::ConstMaxSize>::MAX_SIZE,
    );
}

#[test]
fn has_the_spec_type_of_a_contract_address() {
    assert_eq!(
        <Address<TokenClient> as ContractTypeRefSpec>::INFO.ty,
        <ContractAddress as ContractTypeRefSpec>::INFO.ty,
    );
}

#[test]
fn has_the_topic_of_a_contract_address() {
    let typed: Address<TokenClient> = TOKEN.into();
    assert_eq!(
        Topic::from(typed.into_topic()),
        Topic::from(TOKEN.into_topic()),
    );
}

#[test]
fn converts_and_compares_with_a_contract_address() {
    let typed = Address::<TokenClient>::new(TOKEN);
    assert_eq!(typed, TOKEN);
    assert_eq!(TOKEN, typed);
    assert_eq!(*typed, TOKEN);
    assert_eq!(typed.address(), &TOKEN);
    assert_eq!(ContractAddress::from(typed), TOKEN);
    assert_eq!(Address::<TokenClient>::from_ref(&TOKEN), &typed);
    assert_eq!(typed.cast::<HolderClient>().into_address(), TOKEN);
    assert_eq!(format!("{typed:?}"), format!("{TOKEN:?}"));
    assert_eq!(
        Address::<TokenClient>::default(),
        ContractAddress::default()
    );
}

#[test]
fn lists_its_interface_ids_in_order() {
    let token = <TokenClient as ContractClient>::INTERFACE_ID;
    let named = <MetadataClient as ContractClient>::INTERFACE_ID;
    assert_ne!(token, named);
    assert_eq!(Address::<TokenClient>::INTERFACE_IDS, &[token]);
    assert_eq!(
        Address::<(TokenClient, MetadataClient)>::INTERFACE_IDS,
        &[token, named]
    );
    assert_eq!(
        Address::<(MetadataClient, TokenClient, HolderClient)>::INTERFACE_IDS,
        &[named, token, <HolderClient as ContractClient>::INTERFACE_ID]
    );
    assert_eq!(Address::<(MetadataClient,)>::INTERFACE_IDS, &[named]);
    // Eight is the most one address lists; repeats only matter to lookups.
    type Eight = (
        TokenClient,
        MetadataClient,
        TokenClient,
        MetadataClient,
        TokenClient,
        MetadataClient,
        TokenClient,
        MetadataClient,
    );
    assert_eq!(
        Address::<Eight>::INTERFACE_IDS,
        &[token, named, token, named, token, named, token, named]
    );
}

#[test]
fn several_clients_share_one_address() {
    run(|| {
        let both = Address::<(TokenClient, MetadataClient)>::new(TOKEN);
        // The representation does not depend on the client list.
        assert_eq!(
            core::mem::size_of_val(&both),
            core::mem::size_of::<ContractAddress>()
        );
        assert_eq!(cxs::encode(&both).unwrap(), cxs::encode(&TOKEN).unwrap());

        // Named by the client, by annotation, and by turbofish.
        assert_eq!(TokenClient::at(&both).label(), 42);
        assert_eq!(MetadataClient::at(&both).name_len(), 5);
        let named: MetadataClient = both.client();
        assert_eq!(named.name_len(), 5);
        assert_eq!(both.client::<TokenClient, _>().label(), 42);

        // Narrowing keeps one listed client.
        let token: Address<TokenClient> = both.narrow();
        assert_eq!(token.client().label(), 42);
        assert_eq!(TokenClient::at(&token).label(), 42);

        let holder = HolderClient::new(&HOLDER);
        holder.set_both(&both);
        assert_eq!(holder.label_plus_name_len(), 47);
    });
}

#[test]
fn stored_address_gives_the_client() {
    run(|| {
        let token = Address::<TokenClient>::new(TOKEN);
        assert_eq!(token.client().label(), 42);

        let holder = HolderClient::new(&HOLDER);
        let terms = Terms { token, amount: 7 };
        holder.set_terms(terms.clone());
        holder.set_token(&token);

        assert_eq!(holder.terms(), terms);
        assert_eq!(holder.token(), token);
        assert_eq!(holder.listed(&TOKEN), 7);
        assert_eq!(holder.count(&[token, token]), 2);
        assert_eq!(holder.label_from_terms(), 42);
        assert_eq!(holder.label_from_token(), 42);
    });
}
