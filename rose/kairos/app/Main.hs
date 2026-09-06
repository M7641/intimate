-- | A runnable tour of the library. @cabal run kairos@ prints it.
module Main (main) where

import Data.Maybe (fromJust)
import Kairos.Interval (Interval, interval)
import Kairos.Allen (relate)
import Kairos.Bitemporal (BiTemporalFact(..), asOf)

-- | A convenience for the demo only. 'fromJust' is partial — it crashes on
-- 'Nothing' — so it has no place in library code, but here every interval is
-- a literal we can see is well-formed.
mk :: Int -> Int -> Interval Int
mk s e = fromJust (interval s e)

main :: IO ()
main = do
  putStrLn "== Allen relations =="
  let a = mk 1 5
      b = mk 5 9
      c = mk 3 7
  putStrLn $ "  [1,5) vs [5,9): " ++ show (relate a b)  -- Meets
  putStrLn $ "  [1,5) vs [3,7): " ++ show (relate a c)  -- Overlaps
  putStrLn $ "  [3,7) vs [1,5): " ++ show (relate c a)  -- OverlappedBy (the mirror)

  putStrLn ""
  putStrLn "== Bi-temporal query =="
  -- An employee's salary, recorded on two timelines:
  --                            valid time | transaction time | payload
  let facts =
        [ BiTemporalFact (mk 1 100) (mk 1  10)  "salary=50k"   -- first belief
        , BiTemporalFact (mk 5 100) (mk 10 100) "salary=60k"   -- a later correction
        ]
  -- Same world-time question (world@7), different "what did we know when":
  putStrLn $ "  known@5,  world@7  -> " ++ show (asOf 5  7 facts)  -- ["salary=50k"]
  putStrLn $ "  known@20, world@7  -> " ++ show (asOf 20 7 facts)  -- ["salary=60k"]
