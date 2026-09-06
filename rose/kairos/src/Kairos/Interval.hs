-- | A proper, half-open time interval @[start, end)@.
--
-- The whole point of this module is the /export list/ below: we expose the
-- type 'Interval' but NOT its data constructor. So the only way to build an
-- interval anywhere in the program is through 'interval', which refuses to
-- create a malformed one. The invariant @start < end@ is therefore enforced
-- by the type system, not by hopeful discipline at every call site.
module Kairos.Interval
  ( Interval        -- the type is exported; its constructor is not
  , interval        -- the only way in: a "smart constructor"
  , start
  , end
  , member
  ) where

-- | A time interval. Polymorphic in the instant type @a@ (an 'Int' tick, a
-- @UTCTime@, anything 'Ord'). The fields are positional: @Interval s e@.
data Interval a = Interval a a
  deriving (Eq, Show)

-- | Build an interval, or 'Nothing' if @s@ is not strictly before @e@.
--
-- Returning 'Maybe' makes the failure a value the caller must handle — there
-- is no way to "forget" that construction can fail, because the type says so.
interval :: Ord a => a -> a -> Maybe (Interval a)
interval s e
  | s < e     = Just (Interval s e)
  | otherwise = Nothing

-- | The start instant (inclusive).
start :: Interval a -> a
start (Interval s _) = s

-- | The end instant (exclusive).
end :: Interval a -> a
end (Interval _ e) = e

-- | Does an instant fall inside the interval? Half-open: @start@ is in, @end@
-- is out. This is the degenerate "point inside interval" case of the Allen
-- relations, and it is all the bi-temporal point query needs.
member :: Ord a => a -> Interval a -> Bool
member x (Interval s e) = s <= x && x < e
