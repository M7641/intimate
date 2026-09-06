import gleam/otp/actor
import gleeunit
import gleeunit/should
import glisen/worker

pub fn main() {
  gleeunit.main()
}

pub fn worker_starts_with_zero_test() {
  let assert Ok(started) = worker.start(0)
  let count = actor.call(started.data, waiting: 1000, sending: worker.GetCount)
  count |> should.equal(0)
}

pub fn worker_increment_test() {
  let assert Ok(started) = worker.start(1)
  let subject = started.data

  actor.send(subject, worker.Increment)
  actor.send(subject, worker.Increment)
  actor.send(subject, worker.Increment)

  let count = actor.call(subject, waiting: 1000, sending: worker.GetCount)
  count |> should.equal(3)
}
