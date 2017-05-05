irc-bot.rs [![Docs.rs][b-docs]][docs]
===

A library for writing [Internet Relay Chat (IRC) bots] in the programming
language [Rust], additionally providing a pre-configured bot for immediate
use.

[Internet Relay Chat (IRC) bots]: <https://en.wikipedia.org/wiki/IRC_bot>
[Rust]: <https://www.rust-lang.org>

What documentation there is should be available [on Docs.rs][docs].

[docs]: <https://docs.rs/irc-bot>
[b-docs]: <https://docs.rs/irc-bot/badge.svg>


Quick-start
---

To use this library without writing one's own bot with it, run the provided
program `src/bin/egbot.rs`:

    $ # For most people:
    $ cargo run
    $ # For NixOS users:
    $ make run

The name `egbot` is derived from ["e.g."], which means "for example", and is
also a pun on the name of [Eggdrop], an old IRC bot.

["e.g."]: <https://en.wiktionary.org/wiki/e.g.>
[Eggdrop]: <https://en.wikipedia.org/wiki/Eggdrop>

The bot can be configured by editing the [JSON] file `config.json`, which is
passed through to [the `irc` crate], which documents its configuration options
[in its README file][`irc` config]. One should at least add one's IRC nick to
the `owners` field — e.g., if one's nick is ["Ferris"]:

    {
      "owners": ["Ferris"],
      ...
    }

[JSON]: <https://en.wikipedia.org/wiki/JSON>
[the `irc` crate]: <https://crates.io/crates/irc>
[`irc` config]: <https://github.com/aatxe/irc#configuration>
["Ferris"]: <http://www.rustacean.net>


Building
---

For most users, it should suffice to simply use **[Cargo]**:

    $ cargo build

[Cargo]: <http://doc.crates.io>

Users of the Linux distribution **[NixOS]** may prefer to use the provided
[`Makefile`], which wraps the tool `nix-shell`:

    $ make build

[NixOS]: <https://nixos.org>
[`Makefile`]: <https://github.com/8573/irc-bot.rs/blob/master/Makefile>
