extern crate irc;

use irc::client::prelude::*;
use std::io;
use std::path::Path;

error_chain! {
    foreign_links {
        Io(io::Error);
    }

    errors {
        IdentificationFailure(io_err: io::Error)
    }
}

struct State<'a> {
    server: &'a IrcServer,
    addressee_suffix: String,
    chars_indicating_msg_is_addressed_to_nick: Vec<char>,
}

pub fn run<P, ErrF>(irc_config_json_path: P, mut error_handler: ErrF, _modules: Vec<()>)
    where P: AsRef<Path>,
          ErrF: FnMut(Error)
{
    let server = match IrcServer::new(irc_config_json_path) {
        Ok(s) => s,
        Err(e) => return error_handler(e.into()),
    };

    match server.identify().map_err(|err| ErrorKind::IdentificationFailure(err)) {
        Ok(()) => {}
        Err(e) => return error_handler(e.into()),
    };

    State::new(&server).run(error_handler)
}


impl<'a> State<'a> {
    pub fn new(server: &'a IrcServer) -> State<'a> {
        State {
            server: &server,
            addressee_suffix: ": ".into(),
            chars_indicating_msg_is_addressed_to_nick: vec![':', ','],
        }
    }

    pub fn run<ErrF>(&mut self, mut error_handler: ErrF)
        where ErrF: FnMut(Error)
    {
        for msg in self.server.iter() {
            handle_msg(self, msg).unwrap_or_else(|err| error_handler(err))
        }
    }

    fn say(&self, target: &str, addressee: &str, msg: &str) -> Result<()> {
        let final_msg = format!("{}{}{}",
                                addressee,
                                if addressee.is_empty() {
                                    ""
                                } else {
                                    &self.addressee_suffix
                                },
                                msg);
        info!("Sending message to {:?}: {:?}", target, final_msg);
        self.server.send_privmsg(target, &final_msg)?;
        Ok(())
    }

    fn nick(&self) -> &str {
        self.server.current_nickname()
    }

    fn have_owner(&self, nick: &str) -> bool {
        match self.server.config().owners {
            Some(ref vec) => vec.iter().any(|owner| owner == nick),
            None => false,
        }
    }
}

fn handle_msg(state: &mut State, input_msg: io::Result<Message>) -> Result<()> {
    let raw_msg = match input_msg {
        Ok(m) => m,
        Err(e) => bail!(e),
    };

    debug!("{:?}", raw_msg);

    (match raw_msg.command {
         Command::PRIVMSG(..) => handle_privmsg,
         Command::NOTICE(..) => ignore_msg,
         _ => ignore_msg,
     })(state, raw_msg)
}

fn handle_privmsg(state: &State, raw_msg: Message) -> Result<()> {
    let Message { ref tags, ref prefix, ref command } = raw_msg;

    let (target, msg) = match parse_msg_to_nick(state, command, state.nick()) {
        Some((t, m)) => (t, m),
        None => return Ok(()),
    };

    info!("{:?}", raw_msg);

    let sender = parse_prefix(prefix);

    let (reply_target, reply_addressee) = if target != state.nick() {
        (target.as_ref(), sender.nick.unwrap_or(""))
    } else {
        (sender.nick.unwrap_or(target), "")
    };

    let reply = |reply_msg| state.say(reply_target, reply_addressee, reply_msg);

    if msg.is_empty() {
        return reply("Yes?");
    }

    if state.have_owner(sender.nick.unwrap_or("")) {
        if msg.starts_with("join ") {
            state.server.send_join(msg.trim_left_matches("join "))?
        }
    }

    Ok(())
}

fn ignore_msg(_: &State, _: Message) -> Result<()> {
    Ok(())
}

fn is_msg_to_nick(state: &State, target: &str, msg: &str, nick: &str) -> bool {
    target == nick || msg == nick ||
    (msg.starts_with(nick) &&
     (msg.find(|c: char| state.chars_indicating_msg_is_addressed_to_nick.contains(&c)) ==
      Some(nick.len())))
}

fn user_msg(cmd: &Command) -> Option<(&String, &String)> {
    match cmd {
        &Command::PRIVMSG(ref target, ref msg) |
        &Command::NOTICE(ref target, ref msg) => Some((target, msg)),
        _ => None,
    }
}

fn parse_msg_to_nick<'c>(state: &State,
                         cmd: &'c Command,
                         nick: &str)
                         -> Option<(&'c String, &'c str)> {
    user_msg(cmd).and_then(|(target, msg)| if is_msg_to_nick(state, target, msg, nick) {
                               Some((target,
                  msg.trim_left_matches(nick)
                      .trim_left_matches(|c: char| {
                                             state.chars_indicating_msg_is_addressed_to_nick
                                                 .contains(&c)
                                         })
                      .trim()))
                           } else {
                               None
                           })
}

#[derive(Default, Eq, PartialEq)]
struct MsgPrefix<'a> {
    nick: Option<&'a str>,
    user: Option<&'a str>,
    host: Option<&'a str>,
}

fn parse_prefix(prefix: &Option<String>) -> MsgPrefix {
    let prefix = match prefix {
        &Some(ref s) => s,
        &None => return MsgPrefix::default(),
    };
    let mut iter = prefix.rsplitn(2, '@');
    let host = iter.next();
    let mut iter = iter.next().unwrap_or("").splitn(2, '!');
    let nick = iter.next();
    let user = iter.next();
    MsgPrefix {
        nick: nick,
        user: user,
        host: host,
    }
}
