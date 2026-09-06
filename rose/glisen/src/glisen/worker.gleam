import gleam/erlang/process.{type Subject}
import gleam/otp/actor

pub type State {
  State(id: Int, count: Int)
}

pub type Message {
  Increment
  GetCount(reply_to: Subject(Int))
  Crash
}

pub fn start(id: Int) -> Result(actor.Started(Subject(Message)), actor.StartError) {
  actor.new(State(id: id, count: 0))
  |> actor.on_message(handle_message)
  |> actor.start
}

fn handle_message(
  state: State,
  msg: Message,
) -> actor.Next(State, Message) {
  case msg {
    Increment -> actor.continue(State(..state, count: state.count + 1))

    GetCount(reply_to) -> {
      process.send(reply_to, state.count)
      actor.continue(state)
    }

    Crash -> {
      // The supervisor detects this abnormal exit and restarts the worker.
      // The new worker starts fresh with count=0.
      actor.stop_abnormal("Deliberate crash for demo")
    }
  }
}
