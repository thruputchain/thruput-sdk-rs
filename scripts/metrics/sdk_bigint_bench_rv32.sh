#!/bin/bash
set -euo pipefail

# METRIC: per-operation rv32 codesize and runtime gas for each 256-bit
# integer implementation under test.
#
# Not wired into scripts/metrics.sh. Two reasons, both for whoever adds it:
#
#   1. It builds the benchmark contract once per (backend, operation) pair --
#      34 rv32 builds with LTO today -- so it belongs in a job that expects to
#      spend minutes, not alongside the per-commit codesize scrape.
#   2. metrics.sh pushes to the shared groundhogchain/metrics_history repo, and
#      a new metric name there is a decision about that repo's layout.
#
# Until then: run it by hand, or add
#
#     produce_metric sdk_bigint_bench_rv32
#
# to scripts/metrics.sh. The output format is already what metrics_history
# expects -- one `<name> key=value ...` line per build, append-only, same shape
# as sdk_codesizes_rv32.sh.
#
# Row name is `<backend>.<operation>`. Fields:
#
#   text, rodata, data, bss   ELF section sizes of the built contract
#   data_image                rodata + data (the stored initial data image)
#   rv32a                     size of the lowered artifact, the deployed bytes
#   text_net                  text minus this backend's `none` build: the
#                             marginal cost of linking the operation
#   gas_total, gas_base       gas for the kernel call, and for the `none` build
#   gas_net, gas_per_op       gas_total - gas_base, and that over `iters`
#   iters                     kernel iterations behind gas_total
#
# The gas fields are absent when the sweep ran with -s (codesize only), rather
# than zeroed -- a zero would read as a measurement.

# stdout carries the metric rows; the comparison table goes to stderr, which
# metrics.sh discards. Same split as sdk_perf_gas.sh.
./scripts/bigint-bench.sh "$@"
