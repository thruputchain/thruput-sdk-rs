#![allow(dead_code)]

use sdk::prelude::*;

// #[contract]
pub trait IERC20 {
    // event Approval(address indexed owner, address indexed spender, uint value);
    // event Transfer(address indexed from, address indexed to, uint value);

    /*
    function name() external view returns (string memory);
    function symbol() external view returns (string memory);
    function decimals() external view returns (uint8);
    function totalSupply() external view returns (uint);
    function balanceOf(address owner) external view returns (uint);
    function allowance(address owner, address spender) external view returns (uint);

    function approve(address spender, uint value) external returns (bool);
    */

    fn transfer(&self, to: &ContractAddress, amount: u128) -> bool;

    fn balance_of(&self, address: &ContractAddress) -> u128;

    // function transferFrom(address from, address to, uint value) external returns (bool);
}
