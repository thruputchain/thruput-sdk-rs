mod groundhog;
mod stellar;

use groundhog::Groundhog;
use stellar::Stellar;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Actor {
    Admin,
    Alice,
    Bob,
    Spender,
}
use Actor::*;
const ORACLE: &str = "Soroban SDK 28.0.0 / SAC host 28.0.2";
const ACTORS: [Actor; 4] = [Admin, Alice, Bob, Spender];

#[derive(Clone, Copy, Debug)]
enum Op {
    Approve(i128, u32),
    Transfer(i128),
    Spend(i128),
    Burn(i128),
    BurnFrom(i128),
    Authorize(Actor, bool),
    Mint(Actor, i128),
    Clawback(i128),
}
use Op::*;
impl Op {
    fn authorizer(self) -> Actor {
        match self {
            Approve(..) | Transfer(..) | Burn(..) => Alice,
            Spend(..) | BurnFrom(..) => Spender,
            Authorize(..) | Mint(..) | Clawback(..) => Admin,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    Auth,
    IssuerAuth,
    Negative,
    Allowance,
    Balance,
    Overflow,
    Frozen,
    Expiry,
}
use Failure::*;

#[derive(Debug, PartialEq, Eq)]
enum Effect {
    Approval(i128, u32),
    Revocation,
    Transfer(i128),
    Burn(i128),
    Authorization(Actor, bool),
    Mint(Actor, i128),
    Clawback(i128),
}

#[derive(Debug, PartialEq, Eq)]
struct State {
    balances: [i128; 4],
    allowance: i128,
    authorized: [bool; 4],
}

struct Trace {
    name: &'static str,
    gh: Groundhog,
    sac: Stellar,
    expected: State,
    expiry: u32,
    stored_allowance: i128,
    step: usize,
}
impl Trace {
    fn new(name: &'static str, required: bool) -> Self {
        let gh = Groundhog::new(required);
        let sac = Stellar::new(required, gh.height());
        let expected = State {
            balances: [0; 4],
            allowance: 0,
            authorized: [!required; 4],
        };
        let mut trace = Self {
            name,
            gh,
            sac,
            expected,
            expiry: 0,
            stored_allowance: 0,
            step: 0,
        };
        trace.check();
        trace
    }
    fn check(&mut self) {
        let gh = self.gh.state();
        let sac = self.sac.state();
        assert_eq!(
            gh, self.expected,
            "{} step {} Groundhog; {ORACLE}",
            self.name, self.step
        );
        assert_eq!(
            sac, self.expected,
            "{} step {} {ORACLE}",
            self.name, self.step
        );
        assert_eq!(gh, sac);
    }
    fn run(
        &mut self,
        op: Op,
        caller: Option<Actor>,
        failure: Option<Failure>,
        balances: [i128; 4],
        allowance: i128,
    ) {
        self.step += 1;
        eprintln!("{} step {} {op:?}; {ORACLE}", self.name, self.step);
        let gh = self.gh.run(op, caller, failure, self.stored_allowance);
        let sac = self.sac.run(op, caller, failure);
        let effects = if failure.is_some() {
            vec![]
        } else {
            vec![match op {
                Approve(0, _) => Effect::Revocation,
                Approve(n, d) => Effect::Approval(n, d),
                Transfer(n) | Spend(n) => Effect::Transfer(n),
                Burn(n) | BurnFrom(n) => Effect::Burn(n),
                Authorize(a, b) => Effect::Authorization(a, b),
                Mint(a, n) => Effect::Mint(a, n),
                Clawback(n) => Effect::Clawback(n),
            }]
        };
        assert_eq!(gh, effects, "Groundhog effects");
        assert_eq!(sac, effects, "SAC effects");
        if failure.is_none() {
            match op {
                Approve(n, _) => self.stored_allowance = n,
                Spend(n) | BurnFrom(n) => self.stored_allowance -= n,
                _ => {}
            }
        }
        self.expected.balances = balances;
        self.expected.allowance = allowance;
        if failure.is_none()
            && let Authorize(a, b) = op
        {
            self.expected.authorized[a as usize] = b;
        }
        if failure.is_none()
            && let Approve(_, d) = op
        {
            self.expiry = d;
        }
        // Groundhog exposes committed writes only in the next block. Tick both
        // hosts before comparing snapshots; inclusive spending was checked above.
        self.gh.advance();
        self.sac.set_height(self.gh.height());
        if self.gh.height() > self.expiry {
            self.expected.allowance = 0;
        }
        self.check();
    }
    fn ok(&mut self, op: Op, balances: [i128; 4], allowance: i128) {
        self.run(op, Some(op.authorizer()), None, balances, allowance);
    }
    fn fail(&mut self, op: Op, caller: Option<Actor>, failure: Failure) {
        self.run(
            op,
            caller,
            Some(failure),
            self.expected.balances,
            self.expected.allowance,
        );
    }
    fn funded(name: &'static str) -> Self {
        let mut t = Self::new(name, false);
        t.ok(Mint(Alice, 100), [0, 100, 0, 0], 0);
        t
    }
    fn height(&self) -> u32 {
        self.gh.height()
    }
}

#[test]
fn allowance_replacement_exhaustion_expiry_and_revocation() {
    let mut t = Trace::funded("allowances");
    t.ok(Spend(0), [0, 100, 0, 0], 0);
    t.ok(BurnFrom(0), [0, 100, 0, 0], 0);
    let deadline = t.height() + 30;
    t.ok(Approve(60, deadline), [0, 100, 0, 0], 60);
    t.ok(Spend(10), [0, 90, 10, 0], 50);
    t.ok(Approve(25, deadline), [0, 90, 10, 0], 25);
    t.ok(Spend(10), [0, 80, 20, 0], 15);
    t.ok(BurnFrom(15), [0, 65, 20, 0], 0);
    t.fail(Spend(1), Some(Spender), Allowance);
    let inclusive = t.height() + 1;
    t.ok(Approve(20, inclusive), [0, 65, 20, 0], 20);
    assert_eq!(t.height(), inclusive);
    t.ok(Spend(5), [0, 60, 25, 0], 15);
    t.expected.allowance = 0;
    t.check();
    t.ok(Spend(0), [0, 60, 25, 0], 0);
    t.ok(BurnFrom(0), [0, 60, 25, 0], 0);
    t.fail(Spend(1), Some(Spender), Allowance);
    t.fail(BurnFrom(1), Some(Spender), Allowance);
    t.ok(Approve(9, t.height() + 4), [0, 60, 25, 0], 9); // Regrant must discard the expired remainder.
    t.ok(Spend(4), [0, 56, 29, 0], 5);
    t.ok(Approve(0, t.height() + 100), [0, 56, 29, 0], 0);
    let deadline = t.height() + 4;
    t.ok(Approve(7, deadline), [0, 56, 29, 0], 7);
    t.ok(Approve(0, 0), [0, 56, 29, 0], 0);
    t.fail(Spend(1), Some(Spender), Allowance);
    t.fail(Approve(1, t.height() - 1), Some(Alice), Expiry);
    t.ok(Approve(0, 0), [0, 56, 29, 0], 0);
}

#[test]
fn movements_and_failed_calls_preserve_state_and_events() {
    let mut t = Trace::funded("movements");
    t.ok(Transfer(0), [0, 100, 0, 0], 0);
    t.ok(Burn(0), [0, 100, 0, 0], 0);
    t.ok(Approve(150, t.height() + 50), [0, 100, 0, 0], 150);
    t.fail(Spend(101), Some(Spender), Balance);
    t.fail(BurnFrom(101), Some(Spender), Balance);
    t.fail(Transfer(101), Some(Alice), Balance);
    t.fail(Burn(101), Some(Alice), Balance);
    t.fail(Clawback(101), Some(Admin), Balance);
    for amount in [-1, i128::MIN] {
        for op in [
            Approve(amount, t.height() + 10),
            Transfer(amount),
            Spend(amount),
            Burn(amount),
            BurnFrom(amount),
            Mint(Alice, amount),
            Clawback(amount),
        ] {
            t.fail(op, Some(op.authorizer()), Negative);
        }
    }
    t.ok(Transfer(12), [0, 88, 12, 0], 150);
    t.ok(Burn(3), [0, 85, 12, 0], 150);
    t.ok(Spend(7), [0, 78, 19, 0], 143);
    t.ok(BurnFrom(8), [0, 70, 19, 0], 135);
}

#[test]
fn wallet_gateway_and_token_issuer_require_their_own_authorization() {
    let mut t = Trace::funded("authorization");
    t.ok(Approve(20, t.height() + 50), [0, 100, 0, 0], 20);
    for op in [
        Approve(1, t.height() + 30),
        Transfer(1),
        Burn(1),
        Spend(1),
        BurnFrom(1),
        Authorize(Alice, false),
        Mint(Bob, 1),
        Clawback(1),
    ] {
        t.fail(op, None, Auth);
        t.fail(op, Some(Bob), Auth);
    }
    // Bob passes his own wallet gateway; the token must reject him as issuer.
    for op in [Authorize(Alice, false), Mint(Bob, 1), Clawback(1)] {
        t.fail(op, Some(Bob), IssuerAuth);
    }
    t.ok(Spend(1), [0, 99, 1, 0], 19);
}

#[test]
fn required_authorization_freeze_unfreeze_and_clawback() {
    let mut t = Trace::new("issuer policy", true);
    t.fail(Mint(Alice, 100), Some(Admin), Frozen);
    t.ok(Authorize(Alice, true), [0, 0, 0, 0], 0);
    t.ok(Mint(Alice, 100), [0, 100, 0, 0], 0);
    t.ok(Approve(20, t.height() + 50), [0, 100, 0, 0], 20);
    t.fail(Spend(5), Some(Spender), Frozen); // Recipient failure must restore the spent allowance.
    t.ok(Authorize(Bob, true), [0, 100, 0, 0], 20);
    t.ok(Spend(5), [0, 95, 5, 0], 15);
    t.ok(Authorize(Alice, false), [0, 95, 5, 0], 15);
    for op in [Transfer(1), Burn(1), Spend(1), BurnFrom(1)] {
        t.fail(op, Some(op.authorizer()), Frozen);
    }
    t.ok(Approve(30, t.height() + 10), [0, 95, 5, 0], 30);
    t.ok(Approve(0, 0), [0, 95, 5, 0], 0);
    t.ok(Clawback(10), [0, 85, 5, 0], 0);
    t.ok(Authorize(Alice, true), [0, 85, 5, 0], 0);
    t.ok(Transfer(2), [0, 83, 7, 0], 0);
}

#[test]
fn maximum_signed_amounts_can_be_granted_spent_and_burned() {
    let mut spend = Trace::new("maximum delegated transfer", false);
    spend.ok(Mint(Alice, i128::MAX), [0, i128::MAX, 0, 0], 0);
    spend.ok(
        Approve(i128::MAX, spend.height() + 20),
        [0, i128::MAX, 0, 0],
        i128::MAX,
    );
    spend.ok(Spend(i128::MAX), [0, 0, i128::MAX, 0], 0);

    let mut burn = Trace::new("maximum burn", false);
    burn.ok(Mint(Alice, i128::MAX), [0, i128::MAX, 0, 0], 0);
    burn.ok(
        Approve(i128::MAX, burn.height() + 20),
        [0, i128::MAX, 0, 0],
        i128::MAX,
    );
    burn.ok(BurnFrom(i128::MAX), [0, 0, 0, 0], 0);
    // Burning frees native supply capacity as well as the holder's balance.
    burn.ok(Mint(Alice, i128::MAX), [0, i128::MAX, 0, 0], 0);
    burn.ok(Burn(i128::MAX), [0, 0, 0, 0], 0);
}

#[test]
fn native_supply_cap_differs_from_sac_contract_holder_balance_cap() {
    let gh = Groundhog::new(false);
    let mut sac = Stellar::new(false, gh.height());
    let initial_mint = Mint(Alice, i128::MAX);
    let expected_mint = vec![Effect::Mint(Alice, i128::MAX)];
    assert_eq!(gh.run(initial_mint, Some(Admin), None, 0), expected_mint);
    assert_eq!(sac.run(initial_mint, Some(Admin), None), expected_mint);
    gh.advance();
    sac.set_height(gh.height());
    let full = State {
        balances: [0, i128::MAX, 0, 0],
        allowance: 0,
        authorized: [true; 4],
    };
    assert_eq!(gh.state(), full);
    assert_eq!(sac.state(), full);

    // Adding to Alice now fails in both. The runners check concrete
    // TooLarge / SAC OverflowError errors and require no surviving events.
    assert!(
        gh.run(Mint(Alice, 1), Some(Admin), Some(Overflow), 0)
            .is_empty()
    );
    assert!(
        sac.run(Mint(Alice, 1), Some(Admin), Some(Overflow))
            .is_empty()
    );
    gh.advance();
    sac.set_height(gh.height());
    assert_eq!(gh.state(), full);
    assert_eq!(sac.state(), full);

    // Native supply is capped in aggregate; SAC caps these contract holders
    // independently. This is an intentional disagreement, not a shared trace.
    assert!(
        gh.run(Mint(Bob, 1), Some(Admin), Some(Overflow), 0)
            .is_empty()
    );
    assert_eq!(
        sac.run(Mint(Bob, 1), Some(Admin), None),
        [Effect::Mint(Bob, 1)]
    );
    gh.advance();
    sac.set_height(gh.height());
    assert_eq!(gh.state(), full);
    assert_eq!(
        sac.state(),
        State {
            balances: [0, i128::MAX, 1, 0],
            allowance: 0,
            authorized: [true; 4],
        }
    );
}
