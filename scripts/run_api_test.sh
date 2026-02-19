#!/bin/bash
curl -X POST http://127.0.0.1:5530/wakeru \
  -H "Content-Type: application/json; charset=utf-8" \
  -d @./scripts/sample_input_text.json
