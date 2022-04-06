# what is this

NOTE: this is not intended to be production ready as there are few things to improve (and cleanup).

A simple [brightcove](https://www.brightcove.com/en/) proxy.

# why

Brightcove have several kind of apis. In a project we needed to expose some data that
brightcove considers private only - accessible via a jwt token.

# how

In this proxy we authenticate to Brightcove every 4 minutes to refresh the token.
We then fetch the `video_view` attribute from the BC analytics api.

There are two main thread:

- one thread ensure the token is always valid (4 minutes timer)
- one thread serves the requests

## run it

You have be familiar with how BC api works and get a `CLIENT_ID` and `CLIENT_SECRET`.

Create a `.env` file from `.env_sample`.

Then:

```bash
cargo run
```

The server will be listening on port 4000.

Curl it:

```bash
curl --silent "localhost:4000/api/v1/analytics?videos=xxxxxxx598001,xxxxxxx238001" | jq .

{
  "item_count": 1,
  "items": [
    {
      "video": "xxxxxxx598001",
      "video_view": 1
    }
  ]
}

```
