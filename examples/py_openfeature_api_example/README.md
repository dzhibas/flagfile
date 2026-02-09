# Using flags through openfeature API

Init your flags

`ff init`

Serve flags api with

`ff serve`

Then initiate python venv `uv venv` activate with `source .venv/bin/activate` and install deps `uv pip install -r requirements.txt`

run example `uv run test.py` this will use openfeature api to evaluate flags and python openfeature client lib