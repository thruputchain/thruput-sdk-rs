#!/bin/bash

# Print out the spec for erc20-if

exec cargo test -p erc20-if -F spec -- --show-output
