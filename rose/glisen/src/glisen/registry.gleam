import gleam/dict.{type Dict}
import gleam/erlang/process.{type Subject}
import gleam/list
import gleam/otp/actor
import glisen/worker

pub type State {
  State(workers: Dict(Int, Subject(worker.Message)))
}

pub type Message {
  Register(id: Int, subject: Subject(worker.Message))
  GetAll(reply_to: Subject(List(#(Int, Subject(worker.Message)))))
  IncrementWorker(id: Int)
  CrashWorker(id: Int)
  CrashMany(count: Int)
}

// --- Public API ---

pub fn start() -> Result(actor.Started(Subject(Message)), actor.StartError) {
  actor.new(State(workers: dict.new()))
  |> actor.on_message(handle_message)
  |> actor.start
}

pub fn register(
  registry: Subject(Message),
  id: Int,
  subject: Subject(worker.Message),
) -> Nil {
  actor.send(registry, Register(id, subject))
}

pub fn get_all_workers(
  registry: Subject(Message),
) -> List(#(Int, Subject(worker.Message))) {
  actor.call(registry, waiting: 1000, sending: GetAll)
}

pub fn increment_worker(registry: Subject(Message), id: Int) -> Nil {
  actor.send(registry, IncrementWorker(id))
}

pub fn crash_worker(registry: Subject(Message), id: Int) -> Nil {
  actor.send(registry, CrashWorker(id))
}

pub fn crash_many(registry: Subject(Message), count: Int) -> Nil {
  actor.send(registry, CrashMany(count))
}

// --- Handler ---

fn handle_message(
  state: State,
  msg: Message,
) -> actor.Next(State, Message) {
  case msg {
    Register(id, subject) -> {
      let new_workers = dict.insert(state.workers, id, subject)
      actor.continue(State(workers: new_workers))
    }

    GetAll(reply_to) -> {
      process.send(reply_to, dict.to_list(state.workers))
      actor.continue(state)
    }

    IncrementWorker(id) -> {
      case dict.get(state.workers, id) {
        Ok(subject) -> actor.send(subject, worker.Increment)
        Error(_) -> Nil
      }
      actor.continue(state)
    }

    CrashWorker(id) -> {
      case dict.get(state.workers, id) {
        Ok(subject) -> actor.send(subject, worker.Crash)
        Error(_) -> Nil
      }
      actor.continue(state)
    }

    CrashMany(count) -> {
      dict.to_list(state.workers)
      |> list.take(count)
      |> list.each(fn(pair) {
        let #(_, subject) = pair
        actor.send(subject, worker.Crash)
      })
      actor.continue(state)
    }
  }
}
