## rok

A minimal `ngrok` implementation in Rust, for educational purpose.
This work is largely based on [rathole][0], especially [the very first commit][1]. You can read the [`DESIGN.md`](DESIGN.md)
to learn how this is implemented at a high level. Other honorable references are stated [there](DESIGN.md#references).

_This is by no mean an idiomatic or correct Rust implementation. I am learning
Rust and fairly new to writing networking code with `tokio`_.

## Quick Start

There are two components in `rok`:

- `src/server.rs`: the server consists of two part, the control server and the proxy server.
  - The control server take in request from the clients, assign domain name and setup proxy server
  for each clients.
  - The proxy server will receive requests through the provided domain name and proxy it to the client.
running locally in the user machine.
- `src/client.rs`: the client receive the requests from the internet through the server,
and further proxy it to the web server running locally by the user.

### Setting Up

So in order to see this code in action, you'll have to run 3 service:

#### Running the server

The `server` binary expect the `domains` to be provided. These are the domains that
users from the internet will send requests to. Since we are running locally, we could
mock those domains and have it work locally by manually editing our hosts file at
`/etc/hosts`. Hence, this would only work in your local machine

```
# /etc/hosts

127.0.0.1 a.domain.com
127.0.0.1 b.domain.com
127.0.0.1 c.domain.com
```

Then, we can pass these to the `server` and run it:

```bash
RUST_LOG=info cargo run --bin server -- --domains a.domain.com --domains b.domain.com --domains c.domain.com
```

By default, the server start at port 3001.

```
Jul 15 21:10:13.896  INFO server: Listening on TCP: 0.0.0.0:3001
```

You could customize it by passing it into `--port`.

#### Running the client

The client, on the other hand, only required the `--domain-name` to be passed in, which is the
domain/IP for your `rok` server above:

```bash
# Since we are running our server locally, we can just pass in 127.0.0.1
cargo run --bin client -- --domain-name 127.0.0.1
```

Similarly, by default, it assumed the `rok` server is start at port 3001 and can be customize
by passing the value at `--port`. You shall see the following output once it start successfully:

```
tunnel up!
Host: c.domain.com
```

On the `rok` server side, you will see the following logs:

```
Jul 15 21:29:39.131  INFO server: Accpeting new client...
Jul 15 21:29:39.131  INFO server: Accpeting new client...
Jul 15 21:29:39.132  INFO server: Listening to 0.0.0.0:3004
```

This mean that the `rok` server has spin up another server at port 3004 to proxy the requsts for the
above `client`.

#### Running a web server

The last piece of it is a working webserver running locally at port 4000. _(Yes, it is currently
hardcoded at port 4000...)_.

### Playing around with it

Once you have the above servers running, you can now `curl` the provided domain on the client
output and see your requests going through the proxy:

```
# The reason we are hitting port 3004, is because the `rok` server
# spin up the proxy server at port 3004.
curl http://c.domain.com:3004
```

You'll now see the `rok` client show some information about
the request we just send:

```
GET /                9.973482ms      200 OK
```

That's all. Hope you enjoy it. Once again, these only works locally.

If you wish to see it work deploy somewhere to the cloud, let me know. If it gain enough traction, it's definitely a future work I would like to work on.


[0]: https://github.com/rapiz1/rathole
[1]: https://github.com/rapiz1/rathole/commit/8f3bf5c7c7109821d737a6a67a7fd51fdf3b0917
