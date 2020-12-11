use serenity::{
    client::Context,
    framework::standard::{macros::check, Args, CheckResult, CommandOptions},
    model::channel::Message,
};

// A function which acts as a "check", to determine whether to call a command.
//
// In this case, this command checks to ensure you are the owner of the message
// in order for the command to be executed. If the check fails, the command is
// not called.
#[check]
#[name = "Owner"]
async fn owner_check(_: &Context, msg: &Message, _: &mut Args, _: &CommandOptions) -> CheckResult {
    // Replace 7 with your ID to make this check pass.
    //
    // `true` will convert into `CheckResult::Success`,
    //
    // `false` will convert into `CheckResult::Failure(Reason::Unknown)`,
    //
    // and if you want to pass a reason alongside failure you can do:
    // `CheckResult::new_user("Lacked admin permission.")`,
    //
    // if you want to mark it as something you want to log only:
    // `CheckResult::new_log("User lacked admin permission.")`,
    //
    // and if the check's failure origin is unknown you can mark it as such (same as using `false.into`):
    // `CheckResult::new_unknown()`
    (msg.author.id == 212355530474127361).into()
}
