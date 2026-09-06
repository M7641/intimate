
  The Core Problem

  The panic "Cannot start a runtime from within a runtime" happens because:

  1. postgres crate is synchronous - When a postgres::Client is dropped, it sends a TCP termination message to the database server using blocking I/O
  2. Tokio is shutting down - When your main() function returns, tokio begins shutting down its runtime
  3. Timing conflict - If the postgres Client::drop executes during tokio's shutdown, the blocking I/O conflicts with tokio's async machinery

  Why Previous Attempts Failed

  Without our fix, here's what happened:

  4. main() ends
  5. Tokio runtime starts shutting down
  6. PostgresDatabase is dropped
  7. Arc<Pool> reference count goes to 0
  8. Pool is dropped
  9. Pool's Drop tries to close connections
  10. postgres::Client::drop executes ← THIS IS THE PROBLEM
  11. Client::drop does blocking I/O
  12. But we're on a tokio thread during shutdown
  13. PANIC!

  Why the Arc<Mutex<Option>> Pattern Works

  The key insight is we remove the pool BEFORE tokio shuts down:

  The Option is Critical

  pool: Arc<Mutex<Option<r2d2::Pool<PostgresConnectionManager>>>>

  The Option allows us to use .take(), which:
  - Takes ownership of the pool OUT of the Option
  - Replaces it with None
  - Returns the owned pool

  The close() Flow

  async fn close(&self) -> Result<(), DatabaseError> {
      tokio::task::spawn_blocking(move || {
          let mut pool_guard = pool_mutex.lock().unwrap();
          let pool = pool_guard.take(); // ← TAKES the pool, leaves None behind
          drop(pool_guard);

          // Now we OWN the pool in this blocking thread
          if let Some(pool) = pool {
              // Drain connections
              // Drop connections  ← Client::drop happens HERE
              // Drop pool         ← Pool::drop happens HERE
          }
      })
  }

  What Happens After close()

  This is the magic:

  1. db.close().await is called
  2. Pool is TAKEN from the mutex (mutex now contains None)
  3. Pool is moved into spawn_blocking
  4. ALL drops happen inside spawn_blocking thread
  5. close() completes
  6. main() returns
  7. Tokio runtime starts shutting down
  8. PostgresDatabase is dropped
  9. Arc<Mutex<Option<Pool>>> is dropped
  10. But Option contains None - NOTHING TO DROP!
  11. No postgres cleanup happens here - it already happened!
  12. Clean shutdown ✅

  The Blocking Thread is Key

  spawn_blocking gives us a dedicated OS thread from tokio's blocking thread pool. This thread:
  - Is NOT used for async tasks
  - CAN safely do blocking I/O
  - Runs BEFORE tokio shutdown (because we await it)

  So when postgres::Client::drop does its blocking I/O to send termination messages, it's happening on a thread designed for exactly that purpose.

  Why Mutex Specifically

  - Arc - Multiple references (one in PostgresDatabase, one moved to spawn_blocking)
  - Mutex - Safe concurrent access from different threads
  - Option - Allows us to move the pool out, leaving None behind

  Without the Option, we could only get a reference to the pool, not take ownership. We need ownership to drop it in our controlled blocking context.

  The Fundamental Solution

  The solution boils down to:
  1. Explicit cleanup - We control exactly when and where drops happen
  2. Blocking context - All drops happen in spawn_blocking
  3. Before shutdown - We await close(), so it completes before main() returns
  4. Empty container - After close(), the PostgresDatabase has no pool to drop

  This is why calling db.close().await at the end of your example is essential - it ensures all postgres cleanup happens in a safe blocking context before tokio tries to shut down