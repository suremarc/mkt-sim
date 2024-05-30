# exchange

Work in progress. This is planned to be the backend for a financial exchange
with a rudimentary order matching engine.

## Running locally

We recommend building this project using [Earthly](<https://earthly.dev/get-earthly>).

### Building it in Earthly

Requirement:

* [Earthly](<https://earthly.dev/get-earthly>)
* Docker

Run the following commands:

```bash
# Generate the database
earthly +diesel-setup

# Build the docker image
earthly +docker

# Start services
docker compose up
```

## Testing it out

A Swagger UI endpoint is available at `/swagger`. Otherwise, a raw OpenAPI spec is served at `/auth/openapi.json`, `/accounts/openapi.json`, and `assets/openapi.json`.

```bash
# Get auth token
curl localhost:8000/auth/token -d '{"email": "user@example.com", "password":"string"}' > token.txt

# List accounts
curl -H "Authorization: Bearer $(cat token.txt)" localhost:8000/accounts | jq
```

```json
{
  "count": 1,
  "items": [
    {
      "id": "00000000-0000-0000-0000-000000000000",
      "email": "user@example.com",
      "roles": "ADMIN | USER"
    }
  ]
}
```

```bash
# Create a new equity asset
curl -H "Authorization: Bearer $(cat token.txt)" localhost:8000/assets/equities -d '{"items": [{"ticker":"AAPL"}]}' | jq
```

```json
{
  "count": 1,
  "items": [
    {
      "id": 1,
      "created": "2024-05-26T09:27:06",
      "ticker": "AAPL",
      "description": null
    }
  ]
}
```
