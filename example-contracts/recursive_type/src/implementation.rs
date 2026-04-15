extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use sdk::*;

#[contracttype]
pub struct Tree {
    pub value: u32,
    pub left: Option<Box<Tree>>,
    pub right: Option<Box<Tree>>,
}

#[contracttype]
pub struct Forest {
    pub trees: Vec<Forest>,
    pub label: u32,
}

#[derive(Default)]
pub struct RecursiveTypeContract;

#[contract(version = "0.1.0", client = Client)]
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

    pub fn forest_depth(forest: Forest) -> u32 {
        let mut max = 0;
        for child in forest.trees {
            let d = Self::forest_depth(child);
            if d > max {
                max = d;
            }
        }
        1 + max
    }

    pub fn unbox(val: Box<u32>) -> u32 {
        *val
    }
}
