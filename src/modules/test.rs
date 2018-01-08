use core::*;
use core::BotCmdAuthLvl as Auth;
use yaml_rust::Yaml;

pub fn mk() -> Module {
    mk_module("test")
        .command(
            "test-line-wrap",
            "",
            "Request a long message from the bot, to test its line-wrapping function.",
            Auth::Admin,
            Box::new(test_line_wrap),
        )
        .command(
            "test-panic-catching",
            "",
            "This command's handler function panics, to test the bot framework's panic-catching \
             mechanism.",
            Auth::Admin,
            Box::new(test_panic_catching),
        )
        .end()
}

const LOREM_IPSUM_TEXT: &'static str =
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Integer et tincidunt nibh. Nullam \
     aliquet imperdiet cursus. Duis at turpis mollis, iaculis quam sed, efficitur arcu. Sed vel \
     massa sit amet magna efficitur hendrerit. Donec auctor auctor ligula nec semper. Nulla a \
     odio suscipit, suscipit velit in, ullamcorper velit. In bibendum pulvinar ipsum. Fusce \
     elementum maximus mattis. Donec sed mauris nec ante eleifend dapibus non faucibus massa. \
     Vivamus a auctor ligula. Cras hendrerit, velit sit amet sagittis placerat, elit elit feugiat \
     quam, vel aliquet ligula elit sit amet nibh. Fusce dignissim, orci vitae sodales ornare, \
     lacus risus facilisis sem, a imperdiet lectus massa at velit. Etiam sed magna congue, \
     pulvinar diam quis, facilisis risus. Sed semper, lectus vulputate luctus fermentum, quam \
     lacus consectetur arcu, ac mollis ipsum metus vel nunc. Ut posuere arcu enim, id dictum arcu \
     sagittis in. Mauris a lectus nec ligula eleifend rutrum. Class aptent taciti sociosqu ad \
     litora torquent per conubia massa nunc.";

fn test_line_wrap(_: &State, _: &MsgMetadata, _: &Yaml) -> BotCmdResult {
    BotCmdResult::Ok(Reaction::Reply(LOREM_IPSUM_TEXT.into()))
}

fn test_panic_catching(_: &State, _: &MsgMetadata, _: &Yaml) -> BotCmdResult {
    panic!("Panicking for testing purposes....")
}
