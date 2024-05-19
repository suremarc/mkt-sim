# exchange

Work in progress. This is planned to be the backend for a financial exchange
with a rudimentary order matching engine.

## Running locally

You can build this project manually or using [Earthly](<https://earthly.dev/get-earthly>).

### Building it in Earthly

Requirement:

* [Earthly](<https://earthly.dev/get-earthly>)
* Docker

Run the following commands:

```bash
# Generate the database
earthly +sqlx-setup

# Build the docker image
earthly +docker

# Start services
docker compose up
```

### Building it manually

Requirements:

* `docker`
* `cargo` (<https://rustup.rs>)

Run the following commands:

```bash
# Spin up TigerBeetle
docker compose up -d

# Install sqlx cli
cargo install sqlx-cli

# Generate the database
cargo sqlx database setup

# Start the server
cargo run -- serve
```

## Testing it out

Now test out the the API:

```bash
curl localhost:8000/instruments/equities -d '{"ticker":"AAPL"}'
curl -s localhost:8000/instruments/equities/AAPL | jq
```

You should receive the following payload:

```json
{
  "ticker": "AAPL",
  "description": null
}
```
