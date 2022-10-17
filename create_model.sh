#!/bin/bash


cargo run \
  --bin  model-packager \
  -- \
  --crypto-secret "FAKE_SECRET" \
  --acoustic-file foo.txt \
  --vocoder-file bar.txt \
  --model-name "Bill Nye (sad)" \
  --model-token "vcm_ABCDEF012345" \
  --author-username "Vegito" \
  --expires-in-days 30 \
  --output-file final.fakeyou





