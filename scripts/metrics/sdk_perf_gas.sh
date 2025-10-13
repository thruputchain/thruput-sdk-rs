#!/bin/bash

perf_gas_contracts="\
    erc20 \
    erc20_no_storage \
    "

for c in $perf_gas_contracts ; do
    gas_consumed=$(./scripts/make.sh examples metric "$c" metric_perf_gas)

    printf "${c}\tgas_consumed\t${gas_consumed}\n"
done
