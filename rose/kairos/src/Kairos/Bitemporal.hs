-- | A tiny bi-temporal fact store — the bridge from the pure algebra to
-- @saga@'s problem (see ../saga/genisis.md).
--
-- A fact lives on /two independent timelines/:
--
--   * __valid time__       — when the fact was true in the world;
--   * __transaction time__ — when the system believed it.
--
-- Keeping them separate is what lets you ask "what did we /think/ last Monday
-- that yesterday's sales were?" — and get a different answer than "what do we
-- /now/ know yesterday's sales were", after a correction landed. Each timeline
-- is just an 'Interval', so the whole query reduces to interval membership.
module Kairos.Bitemporal
  ( BiTemporalFact(..)
  , asOf
  ) where

import Kairos.Interval (Interval, member)

-- | A value tagged with both timelines. @t@ is the instant type, @v@ the
-- payload (a salary, a price, a status — whatever is being remembered).
data BiTemporalFact t v = BiTemporalFact
  { validTime :: Interval t  -- ^ when it held in the world
  , txTime    :: Interval t  -- ^ when the system believed it
  , payload   :: v
  } deriving (Eq, Show)

-- | Bi-temporal point query. Keep the payloads of every fact that
--
--   * the system believed at transaction time @asKnown@, AND
--   * held in the world at valid time @inWorld@.
--
-- Two interval-membership tests, combined with 'and'. That is the entire
-- query engine — the expressiveness comes from the two timelines, not from
-- machinery.
asOf :: Ord t => t -> t -> [BiTemporalFact t v] -> [v]
asOf asKnown inWorld = map payload . filter match
  where
    match f = asKnown `member` txTime f
           && inWorld `member` validTime f
