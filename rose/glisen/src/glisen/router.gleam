import gleam/erlang/process.{type Subject}
import gleam/http
import gleam/int
import gleam/json
import gleam/list
import gleam/otp/actor
import wisp.{type Request, type Response}
import glisen/registry
import glisen/worker

pub type Context {
  Context(registry: Subject(registry.Message))
}

pub fn handle_request(req: Request, ctx: Context) -> Response {
  use <- wisp.log_request(req)

  case wisp.path_segments(req) {
    [] -> home(req)
    ["workers"] -> list_workers(req, ctx)
    ["workers", id_str, "increment"] -> increment(req, ctx, id_str)
    ["workers", id_str, "crash"] -> crash_one(req, ctx, id_str)
    ["workers", "crash-many", count_str] -> crash_many(req, ctx, count_str)
    _ -> wisp.not_found()
  }
}

fn home(req: Request) -> Response {
  use <- wisp.require_method(req, http.Get)
  wisp.ok()
  |> wisp.string_body(
    "Glisen - BEAM Supervision Demo\n"
    <> "\n"
    <> "GET  /workers                    List all workers\n"
    <> "POST /workers/{id}/increment     Increment a worker's counter\n"
    <> "POST /workers/{id}/crash         Crash a specific worker\n"
    <> "POST /workers/crash-many/{n}     Crash n workers at once\n"
    <> "\n"
    <> "Crashed workers are restarted automatically by the supervisor.\n"
    <> "They come back with count=0. Others keep their state.\n",
  )
}

fn list_workers(req: Request, ctx: Context) -> Response {
  use <- wisp.require_method(req, http.Get)
  let workers = registry.get_all_workers(ctx.registry)

  let worker_data =
    list.map(workers, fn(pair) {
      let #(id, subject) = pair
      let count = actor.call(subject, waiting: 500, sending: worker.GetCount)
      #(id, count)
    })

  let body =
    json.to_string(json.object([
      #(
        "workers",
        json.array(worker_data, fn(pair) {
          let #(id, count) = pair
          json.object([#("id", json.int(id)), #("count", json.int(count))])
        }),
      ),
      #("total", json.int(list.length(worker_data))),
    ]))

  wisp.json_response(body, 200)
}

fn increment(req: Request, ctx: Context, id_str: String) -> Response {
  use <- wisp.require_method(req, http.Post)
  case int.parse(id_str) {
    Ok(id) -> {
      registry.increment_worker(ctx.registry, id)
      wisp.json_response(
        json.to_string(json.object([
          #("action", json.string("incremented")),
          #("worker_id", json.int(id)),
        ])),
        200,
      )
    }
    Error(_) -> wisp.bad_request("Invalid worker ID")
  }
}

fn crash_one(req: Request, ctx: Context, id_str: String) -> Response {
  use <- wisp.require_method(req, http.Post)
  case int.parse(id_str) {
    Ok(id) -> {
      registry.crash_worker(ctx.registry, id)
      wisp.json_response(
        json.to_string(json.object([
          #("action", json.string("crashed")),
          #("worker_id", json.int(id)),
          #("note", json.string("Worker will be restarted by supervisor")),
        ])),
        200,
      )
    }
    Error(_) -> wisp.bad_request("Invalid worker ID")
  }
}

fn crash_many(req: Request, ctx: Context, count_str: String) -> Response {
  use <- wisp.require_method(req, http.Post)
  case int.parse(count_str) {
    Ok(count) -> {
      registry.crash_many(ctx.registry, count)
      wisp.json_response(
        json.to_string(json.object([
          #("action", json.string("crashed_many")),
          #("count", json.int(count)),
          #("note", json.string("Workers will be restarted by supervisor")),
        ])),
        200,
      )
    }
    Error(_) -> wisp.bad_request("Invalid count")
  }
}
