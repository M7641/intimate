-- | Property-based tests: we state the /laws/ of the algebra and let QuickCheck
-- hunt for a counter-example by generating hundreds of random interval pairs.
-- A law is a universally-quantified claim ("for all intervals x, y ...");
-- QuickCheck is how you test one without enumerating every input by hand.
module Main (main) where

import Control.Monad (unless)
import System.Exit (exitFailure)
import Test.QuickCheck
import Kairos.Interval (Interval, interval)
import Kairos.Allen (Relation(..), relate, converse)

-- | A generator of /proper/ intervals: pick a start, then add a positive
-- length so @start < end@ always holds. We funnel through the smart
-- constructor; the 'Nothing' branch is unreachable by construction.
genInterval :: Gen (Interval Int)
genInterval = do
  s            <- arbitrary
  Positive len <- arbitrary
  case interval s (s + len) of
    Just i  -> pure i
    Nothing -> error "genInterval: unreachable — len is positive"

-- | Every relation value, for laws quantified over 'Relation' rather than
-- over intervals.
genRelation :: Gen Relation
genRelation = elements [minBound .. maxBound]

-- Law 1: 'converse' is an involution — mirroring twice gets you home.
prop_converseInvolutive :: Property
prop_converseInvolutive =
  forAll genRelation $ \r -> converse (converse r) == r

-- Law 2: an interval is 'Equal' to itself, and nothing else.
prop_selfEqual :: Property
prop_selfEqual =
  forAll genInterval $ \x -> relate x x == Equal

-- Law 3: the defining symmetry of the algebra —
-- relating x to y is the mirror of relating y to x.
prop_relateConverse :: Property
prop_relateConverse =
  forAll genInterval $ \x ->
  forAll genInterval $ \y ->
    relate x y == converse (relate y x)

main :: IO ()
main = do
  ok <- and <$> mapM checked
    [ ("converse is involutive",        quickCheckResult prop_converseInvolutive)
    , ("an interval equals only itself", quickCheckResult prop_selfEqual)
    , ("relate x y mirrors relate y x", quickCheckResult prop_relateConverse)
    ]
  unless ok exitFailure
  where
    checked (name, run) = do
      putStrLn ("-- " ++ name)
      isSuccess <$> run
