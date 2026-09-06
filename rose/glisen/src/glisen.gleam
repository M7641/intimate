import gleam/erlang/process
import gleam/int
import gleam/io
import gleam/otp/static_supervisor as supervisor
import gleam/otp/supervision
import mist
import wisp
import wisp/wisp_mist
import glisen/registry
import glisen/router
import glisen/worker

const worker_count = 10

pub fn main() {
  wisp.configure_logger()
  io.println("Starting Glisen - BEAM Supervision Demo")

  // 1. Start the registry actor (maps worker IDs → live subjects)
  let assert Ok(registry_started) = registry.start()
  let registry_subject = registry_started.data
  io.println("Registry started")

  // 2. Start the supervisor with N workers (OneForOne = only crashed child restarts)
  let sup =
    int.range(
      from: 0,
      to: worker_count,
      with: supervisor.new(supervisor.OneForOne),
      run: fn(builder, id) {
        supervisor.add(builder, make_worker_child(id, registry_subject))
      },
    )

  let assert Ok(_) = supervisor.start(sup)
  io.println(
    "Supervisor started with "
    <> int.to_string(worker_count)
    <> " workers (OneForOne)",
  )

  // 3. Start HTTP server
  let ctx = router.Context(registry: registry_subject)
  let handler = fn(req) { router.handle_request(req, ctx) }
  let secret = wisp.random_string(64)

  let assert Ok(_) =
    wisp_mist.handler(handler, secret)
    |> mist.new
    |> mist.port(8080)
    |> mist.start

  io.println("Listening on http://localhost:8080")
  io.println("")
  io.println("Try:")
  io.println("  curl http://localhost:8080/workers")
  io.println("  curl -X POST http://localhost:8080/workers/0/increment")
  io.println("  curl -X POST http://localhost:8080/workers/3/crash")
  io.println("  curl http://localhost:8080/workers")

  process.sleep_forever()
}

/// Build a child spec that starts a worker and registers it with the registry.
/// On restart, the supervisor calls this closure again — fresh worker, re-registered.
fn make_worker_child(
  id: Int,
  registry_subject: process.Subject(registry.Message),
) -> supervision.ChildSpecification(process.Subject(worker.Message)) {
  supervision.worker(fn() {
    case worker.start(id) {
      Ok(started) -> {
        registry.register(registry_subject, id, started.data)
        Ok(started)
      }
      Error(e) -> Error(e)
    }
  })
}
