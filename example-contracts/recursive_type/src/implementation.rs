extern crate alloc;

use alloc::boxed::Box;
use sdk::*;

#[contracttype]
pub struct Tree {
    pub value: u32,
    pub left: Option<Box<Tree>>,
    pub right: Option<Box<Tree>>,
}

#[derive(Default)]
pub struct RecursiveTypeContract;

#[contract(version = "0.1.0")]
impl RecursiveTypeContract {
    pub fn depth(tree: Tree) -> u32 {
        let left_depth = match tree.left {
            Some(left) => Self::depth(*left),
            None => 0,
        };
        let right_depth = match tree.right {
            Some(right) => Self::depth(*right),
            None => 0,
        };
        1 + if left_depth > right_depth {
            left_depth
        } else {
            right_depth
        }
    }
}
