"""Tests de la feature `postmacode`, à côté du code (style Rust).

Mélange tests par l'exemple et tests de propriété (Hypothesis) : un test de
propriété affirme un invariant qui doit tenir pour *toutes* les entrées ;
Hypothesis cherche un contre-exemple puis le réduit au plus petit cas.
"""

import string

import pytest
from hypothesis import example, given
from hypothesis import strategies as st

from destiny.postmacode import PostMaCode

# Alphabet réaliste d'un code postal saisi à la main : lettres, chiffres,
# espaces et points (que `clean_postcode` est censé normaliser).
POSTCODE_TEXT = st.text(
    alphabet=string.ascii_letters + string.digits + " .", max_size=12
)


# ── Tests par l'exemple ───────────────────────────────────────────────────────


def test_clean_postcode_inserts_space():
    pmc = PostMaCode()
    assert pmc.clean_postcode("m11ae") == "M1 1AE"
    assert pmc.clean_postcode("sw1a1aa") == "SW1A 1AA"


def test_clean_postcode_already_spaced_is_idempotent():
    pmc = PostMaCode()
    assert pmc.clean_postcode("EC1A 1BB") == "EC1A 1BB"


def test_clean_postcode_strips_whitespace_dots_and_uppercases():
    pmc = PostMaCode()
    assert pmc.clean_postcode("  m1 1ae. ") == "M1 1AE"


def test_clean_postcode_four_char_boundary_inserts_space():
    # Exactly 4 chars, no existing space — pins the `len >= 4` boundary so the
    # `>= 4 → > 4` and `>= 4 → >= 5` mutants get killed (found via mutation testing).
    assert PostMaCode().clean_postcode("AB1C") == "A B1C"


# ── Tests de propriété (Hypothesis) ──────────────────────────────────────────


@given(POSTCODE_TEXT)
def test_clean_postcode_output_is_uppercase(raw):
    """Invariant qui tient pour TOUTE entrée : la sortie est en majuscules."""
    cleaned = PostMaCode().clean_postcode(raw)
    assert cleaned == cleaned.upper()


# Propriété désirable mais NON vérifiée : ce test a trouvé un vrai bug. `strip()`
# s'exécute AVANT `strip(".")`, donc une entrée comme "A ." donne "A " (espace
# final) — et re-nettoyer donne "A". `clean_postcode` n'est donc pas idempotent.
# `@example("A .")` fige le contre-exemple pour un xfail déterministe ; on le
# garde en xfail (plutôt que corriger la logique des codes postaux ici) pour
# documenter la trouvaille — un fix de `clean_postcode` le fera passer.
@pytest.mark.xfail(
    reason="clean_postcode not idempotent on trailing '.': 'A .' -> 'A ' -> 'A'",
    strict=True,
)
@example("A .")
@given(POSTCODE_TEXT)
def test_clean_postcode_is_idempotent(raw):
    """Nettoyer un code déjà nettoyé ne devrait plus le changer (point fixe)."""
    pmc = PostMaCode()
    once = pmc.clean_postcode(raw)
    assert pmc.clean_postcode(once) == once
