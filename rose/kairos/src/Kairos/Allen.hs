-- | Allen's interval algebra: the 13 jointly-exhaustive, pairwise-disjoint
-- relations that can hold between two time intervals.
--
-- This is the mathematical heart of @kairos@. Given any two proper intervals,
-- exactly one of these relations holds — no more, no fewer. That "exactly one"
-- is a theorem, and 'relate' is its constructive proof: a total function that
-- always returns, and returns just one answer.
module Kairos.Allen
  ( Relation(..)
  , relate
  , converse
  ) where

import Kairos.Interval (Interval, start, end)

-- | The 13 Allen relations, read as "X r Y" (how the first interval relates
-- to the second). Six are mirror images of six others; 'Equal' is its own
-- mirror. Deriving 'Enum' and 'Bounded' lets us enumerate all of them with
-- @[minBound .. maxBound]@ — handy for exhaustive law checks.
data Relation
  = Before        -- ^ X ends before Y starts, with a gap      (X..X  Y..Y)
  | After         -- ^ converse of 'Before'
  | Meets         -- ^ X ends exactly where Y starts           (X..XY..Y)
  | MetBy         -- ^ converse of 'Meets'
  | Overlaps      -- ^ X starts first, they share a middle     (X..[XY]..Y)
  | OverlappedBy  -- ^ converse of 'Overlaps'
  | Starts        -- ^ same start, X ends first                (X is a prefix of Y)
  | StartedBy     -- ^ converse of 'Starts'
  | During        -- ^ X sits strictly inside Y
  | Contains      -- ^ converse of 'During'
  | Finishes      -- ^ same end, X starts later                (X is a suffix of Y)
  | FinishedBy    -- ^ converse of 'Finishes'
  | Equal         -- ^ identical intervals (self-converse)
  deriving (Eq, Show, Enum, Bounded)

-- | Classify how interval @x@ relates to interval @y@.
--
-- The body is one chain of guards over the four endpoints. Because both
-- intervals are proper (@a < b@, @c < d@, guaranteed by the smart
-- constructor), comparing the endpoints is enough to pin down the relation.
-- Read it top to bottom: the equality cases are tested before the strict
-- inequalities they would otherwise be subsumed by.
relate :: Ord a => Interval a -> Interval a -> Relation
relate x y
  | a == c && b == d = Equal
  | b == c           = Meets
  | d == a           = MetBy
  | b <  c           = Before
  | d <  a           = After
  | a == c && b <  d = Starts
  | a == c && b >  d = StartedBy
  | b == d && a >  c = Finishes
  | b == d && a <  c = FinishedBy
  | a >  c && b <  d = During
  | a <  c && b >  d = Contains
  | a <  c && b <  d = Overlaps        -- a < c < b < d
  | otherwise        = OverlappedBy    -- c < a < d < b
  where
    a = start x; b = end x
    c = start y; d = end y

-- | The mirror of a relation: @relate x y == converse (relate y x)@.
-- This is a total bijection on 'Relation' and is its own inverse.
converse :: Relation -> Relation
converse r = case r of
  Before       -> After
  After        -> Before
  Meets        -> MetBy
  MetBy        -> Meets
  Overlaps     -> OverlappedBy
  OverlappedBy -> Overlaps
  Starts       -> StartedBy
  StartedBy    -> Starts
  During       -> Contains
  Contains     -> During
  Finishes     -> FinishedBy
  FinishedBy   -> Finishes
  Equal        -> Equal
