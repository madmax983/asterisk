# Asterisk

*A language family that drifts while you watch.*

Historical linguists write the words nobody ever heard with a star in front:
`*pəter`, `*wódr̥`, `*mēns`. A starred form is a reconstruction — a word worked
backwards out of its descendants. Asterisk runs the arrow the other way.

It invents a proto-language from a seed — a small inventory, a few syllable
shapes, fifty-odd basic words (*sun, moon, water, tongue, mother, ash, salt,
honey*) — and then lets time happen to it. Every century each living language
may undergo a **sound change**, written in the notation linguists actually use:

    p > f / #_          word-initial p becomes f        (Grimm)
    k > tʃ / _[+front]  k becomes ch before front vowels (Italian, English)
    V > ∅ / C_#         final vowels fall off            (apocope)
    P > [+cont] / V_V   stops soften between vowels      (Spanish lenition)
    ɛ > jɛ              open e breaks into a diphthong    (Castilian *tierra*)

Languages split. Daughters drift apart. Words worn down to one sound get
renewed by compounding ("sun" becomes "little-sun", exactly how Latin *sol*
became French *soleil*). Now and then a language borrows a word from a
sister. Some die. The family tree grows across the screen, and at every node
you can see what the word for *moon* was then.

## The moment

You pick *moon*. Twelve living languages say `mīn`, `vena`, `ɲẽ`, `mɔː`,
`ˈmeŋa`… and they all came from one starred word on the left edge. You click
`ɲẽ` and get its whole life, century by century: `*mena > meɲa` (palatalized
before a front vowel) `> meɲ` (final vowel lost) `> mẽɲ` (nasalized) `> ɲẽ`…
and every step names the rule that did it.

Then you type your own rule — `m > ∅ / #_`, say — into the workbench, point
it at one language, and press preview. It shows you exactly which nine words
it would wreck. You apply it anyway. Every browser watching sees that
language lurch, and from now on its etymologies carry your rule, struck in
your colour.

## Why it is delightful

* **It surprises.** You cannot predict what a dozen random, plausible sound
  changes do to a word; the results always look like real languages anyway.
* **It is alive.** The world ticks a century every few seconds, forever, and
  every browser sees the same drift over SSE.
* **The core is an engine.** A phonological feature system, a sound-change
  language with a parser, a printer and a simultaneous-application
  semantics, and a stochastic tree of languages built on top of it.
* **You can get lost in it.** Every word in every language has a provable
  ancestry, and the ancestry is the fun part.

## What is guaranteed

Every word in every language, at every century, after any sequence of sound
changes — drifted or hand-typed — is a non-empty string of at most twelve
segments drawn from the inventory. The application step that guarantees it is
modelled and proved in Verus. The rule language round-trips: printing a rule
and parsing it gives back the same rule (property-tested over arbitrary
rules). And every etymology replays: re-applying a language's recorded
history to the proto-language reproduces its current lexicon exactly.

## Where it could go

* A map: daughters migrate across a generated coastline and borrow from their
  neighbours, so geography shapes the tree.
* The **comparative method** in reverse — show the player only the living
  languages and let them reconstruct the starred forms, scored against the
  truth.
* Grammar, not just words: affixes that grammaticalise and erode.
* Speech: a formant synthesiser so you can *hear* *moon* drift.
