//! Code health scoring.
//!
//! The metrics engine answers "how complex is this function?". This module
//! answers "is this function healthy?", by folding the metrics that have
//! evidence behind them into a single quality percentage, at three
//! granularities: function, file, and repository.
//!
//! # The four pillars
//!
//! Each pillar takes the **worst** of its measures, not their average. Two
//! metrics of the same property are partly redundant, so averaging them
//! would let a function hide a bad number behind a good one, while giving
//! each its own pillar would count one property twice.
//!
//! | Pillar | Measures | Default threshold | Why |
//! | --- | --- | --- | --- |
//! | Control flow | cognitive complexity | 15 | [\[1\]][1] [\[2\]][2] |
//! | | cyclomatic complexity | 10 | [\[3\]][3] [\[4\]][4] |
//! | Size | statements (logical lines) | 30 | [\[5\]][5] [\[6\]][6] |
//! | Vocabulary load | working memory | 8 | [\[7\]][7] [\[8\]][8] [\[9\]][9] |
//! | | Halstead difficulty | 12 | [\[9\]][9] [\[10\]][10] |
//! | Interface | parameters | 4 | [\[5\]][5] [\[6\]][6] [\[11\]][11] |
//!
//! **Control flow.** Cognitive complexity is the only solely code-based
//! metric that has been validated against measured understandability: a
//! meta-analysis over roughly 24,000 understandability evaluations of 427
//! snippets found it correlates with comprehension time and with subjective
//! ratings [\[2\]][2]. Cyclomatic complexity joins it because the claim that
//! it merely restates size does not hold at the granularity scored here —
//! over 17.8M Java methods and 6.3M C functions, its correlation with source
//! lines is only moderate per method, and the near-perfect correlations in
//! the literature come from aggregating over files or systems [\[4\]][4].
//! McCabe's own recommended upper bound of 10 is the threshold [\[3\]][3].
//!
//! **Size.** Unit size is one of the source-code properties in the SIG
//! maintainability model [\[5\]][5], whose certification criteria place the
//! unit-size risk boundaries at 15, 30 and 60 lines [\[6\]][6]. The middle
//! one is the threshold here.
//!
//! **Vocabulary load.** Working-memory capacity is estimated at about four
//! chunks under conditions that prevent chunking [\[8\]][8], against Miller's
//! earlier seven [\[7\]][7]; the threshold sits at the top of that range
//! because a programmer reading familiar code does get to chunk. An fMRI
//! study found that a snippet's vocabulary size, rather than its textual
//! size, is what loads working memory [\[9\]][9], which is why Halstead
//! difficulty — a function of how many distinct operands are reused
//! [\[10\]][10] — measures the same pillar.
//!
//! **Interface.** Parameter count is the SIG model's unit-interfacing
//! measure, with a low-risk bound of 2 [\[5\]][5] [\[6\]][6]; the "long
//! parameter list" smell is conventionally called at 3 to 4. Of the smells
//! studied across five production Python projects, it showed the highest
//! correlation with defects [\[11\]][11]. Its default weight is half the
//! others: a wide signature is a real signal, but a narrower one than the
//! three pillars that describe what a function does.
//!
//! # The class pillar
//!
//! A class is not a function and none of the four pillars is defined for
//! it, so a class is scored on one pillar of its own.
//!
//! | Pillar | Measures | Default threshold | Why |
//! | --- | --- | --- | --- |
//! | Class design | weighted methods per class | 34 | [\[16\]][16] |
//! | | public methods | 14 | [\[16\]][16] [\[17\]][17] |
//! | | public attributes | 8 | [\[16\]][16] [\[17\]][17] |
//!
//! The thresholds come from one benchmark catalogue derived from the 111
//! Java systems of the Qualitas.class Corpus, which splits each metric into
//! common, casual and uncommon ranges [\[16\]][16]; the threshold here is
//! where uncommon starts — WMC above 34, more than 14 methods, more than 8
//! fields. Public methods and public attributes are subsets of the methods
//! and fields the catalogue counts, so reading its boundaries for them is
//! deliberately conservative;
//! how much of a class is public is the older question the two metrics were
//! defined to answer [\[17\]][17]. The same catalogue puts the parameter
//! boundaries at 2 and 4, which is where the interface pillar already sits.
//!
//! A class is only scored where the engine actually computes these metrics,
//! which today means Java. A `class` space in a language whose `Wmc`, `Npm`
//! and `Npa` implementations are no-ops would score a constant 100 and say
//! nothing, so it is left out of the report entirely — and with it out, a
//! file in that language scores exactly what it scored before classes were
//! scored at all.
//!
//! # The formula
//!
//! Each measure is scored on its own before anything is blended, so a
//! function is never rescued by being short if it is impenetrable. A raw
//! value $r \ge 0$ with threshold $t > 0$ scores
//!
//! $$ s(r) = \frac{100}{1 + \left(\dfrac{r}{t}\right)^{2}} $$
//!
//! which gives $s(0) = 100$, $s(t) = 50$, $s(2t) = 20$ and $s(3t) = 10$. The
//! curve is smooth and monotone and never leaves $(0, 100]$, so no clamping
//! is needed and no function is ever written off entirely.
//!
//! A pillar $P$ scores at its worst measure,
//!
//! $$ s_P = \min_{m \in P} s(r_m) $$
//!
//! and the pillars blend into quality as a weighted geometric mean:
//!
//! $$ Q = \prod_{P} s_P^{\,w_P}, \qquad \sum_{P} w_P = 1 $$
//!
//! A geometric mean rather than an arithmetic one, because one collapsed
//! pillar should drag the whole score down: a 200-statement function is hard
//! to work with no matter how flat its control flow is.
//!
//! A class scores at its single pillar, $Q_C = s_{\text{class design}}$. A
//! file blends its functions with its classes the same geometric way,
//!
//! $$ Q_F = Q_{\text{functions}}^{\,1-w} \cdot Q_{\text{classes}}^{\,w},
//!    \qquad w = \frac{w_{\text{class}}}{1 + w_{\text{class}}} $$
//!
//! where $Q_{\text{functions}}$ weights each function by its length and
//! $Q_{\text{classes}}$ weights each class by how many methods it defines.
//! With no scored classes the second factor is absent and $Q_F$ is the
//! function quality untouched.
//!
//! # What is deliberately left out
//!
//! The engine computes more than this module scores. Each omission is a
//! judgement about evidence, not an oversight.
//!
//! - **Maintainability index.** Carried through on [`FileHealth`] because it
//!   is a familiar second opinion, but never scored. Its constants are
//!   curve-fitted to one 1990s corpus, it is computed over averages that
//!   discard the distribution, and it is confounded by size
//!   [\[5\]][5] [\[12\]][12] [\[13\]][13] [\[14\]][14].
//! - **Comment density.** The replicated findings are about whether comments
//!   are *accurate*, not how many there are; raw comment count is a weak
//!   quality proxy and has been found to correlate negatively with other
//!   quality signals [\[15\]][15].
//! - **Number of exit points.** The single-exit rule comes from 1960s
//!   structured programming and resource-cleanup concerns that modern
//!   languages handle. It is defended and attacked in style guides, but a
//!   search of the literature turns up no controlled study relating exit
//!   count to defects or to comprehension.
//! - **Number of methods.** A count on its own, without the complexity
//!   weighting that makes WMC say something about how much a class does.
//!   It is carried on the class report as the weight a file uses to
//!   balance its classes against each other, and not scored.
//! - **ABC.** Computed for Java and for no other grammar this fork carries.
//!   Scoring it would add a second size measure that only one language has,
//!   which would make a Java score mean something different from every
//!   other language's. It is reported, and hidden where it is not computed.
//!
//! # References
//!
//! 1. G. A. Campbell. *Cognitive Complexity: an overview and evaluation.*
//!    Proceedings of the 2018 International Conference on Technical Debt
//!    (TechDebt '18), 57–58. <https://doi.org/10.1145/3194164.3194186>
//! 2. M. Muñoz Barón, M. Wyrich, S. Wagner. *An Empirical Validation of
//!    Cognitive Complexity as a Measure of Source Code Understandability.*
//!    ESEM 2020. <https://doi.org/10.1145/3382494.3410636> — see also
//!    L. Lavazza, A. Z. Abualkishik, G. Liu, S. Morasca, *An empirical
//!    evaluation of the "Cognitive Complexity" measure as a predictor of code
//!    understandability*, JSS 197 (2023), which is more sceptical that it
//!    improves on older measures.
//!    <https://doi.org/10.1016/j.jss.2022.111561>
//! 3. T. J. McCabe. *A Complexity Measure.* IEEE Transactions on Software
//!    Engineering SE-2(4), 1976, 308–320.
//!    <https://doi.org/10.1109/TSE.1976.233837>
//! 4. D. Landman, A. Serebrenik, E. Bouwers, J. J. Vinju. *Empirical analysis
//!    of the relationship between CC and SLOC in a large corpus of Java
//!    methods and C functions.* Journal of Software: Evolution and Process
//!    28(7), 2016, 589–618. <https://doi.org/10.1002/smr.1760>
//! 5. I. Heitlager, T. Kuipers, J. Visser. *A Practical Model for Measuring
//!    Maintainability.* QUATIC 2007, 30–39.
//!    <https://doi.org/10.1109/QUATIC.2007.7>
//! 6. Software Improvement Group / TÜV NORD CERT. *Evaluation Criteria
//!    Trusted Product Maintainability: Guidance for Producers.*
//!    <https://www.softwareimprovementgroup.com/wp-content/uploads/SIG-TUViT-Evaluation-Criteria-Trusted-Product-Maintainability-Guidance-for-producers.pdf>
//!    Thresholds are calibrated by the method of T. L. Alves, C. Ypma,
//!    J. Visser, *Deriving metric thresholds from benchmark data*, ICSM 2010.
//!    <https://doi.org/10.1109/ICSM.2010.5609747>
//! 7. G. A. Miller. *The magical number seven, plus or minus two: some limits
//!    on our capacity for processing information.* Psychological Review
//!    63(2), 1956, 81–97. <https://doi.org/10.1037/h0043158>
//! 8. N. Cowan. *The magical number 4 in short-term memory: a reconsideration
//!    of mental storage capacity.* Behavioral and Brain Sciences 24(1), 2001,
//!    87–114. <https://doi.org/10.1017/S0140525X01003922>
//! 9. N. Peitek, S. Apel, C. Parnin, A. Brechmann, J. Siegmund. *Program
//!    Comprehension and Code Complexity Metrics: An fMRI Study.* ICSE 2021,
//!    524–536. <https://doi.org/10.1109/ICSE43902.2021.00056>
//! 10. M. H. Halstead. *Elements of Software Science.* Elsevier, 1977.
//! 11. F. N. Topuz. *Empirical Evidence of the Consequences of Bad Smells in
//!     Software.* PhD dissertation, Auburn University, 2022.
//!     <https://etd.auburn.edu/handle/10415/8100>
//! 12. A. van Deursen. *Think Twice Before Using the "Maintainability Index".*
//!     2014.
//!     <https://avandeursen.com/2014/08/29/think-twice-before-using-the-maintainability-index/>
//! 13. K. El Emam, S. Benlarbi, N. Goel, S. N. Rai. *The Confounding Effect
//!     of Class Size on the Validity of Object-Oriented Metrics.* IEEE
//!     Transactions on Software Engineering 27(7), 2001, 630–650.
//!     <https://doi.org/10.1109/32.935855>
//! 14. D. I. K. Sjøberg, B. Anda, A. Mockus. *Questioning software
//!     maintenance metrics: a comparative case study.* ESEM 2012, 107–110.
//!     <https://doi.org/10.1145/2372251.2372269>
//! 15. P. Rani, A. Blasi, N. Stulova, et al. *A decade of code comment
//!     quality assessment: A systematic literature review.* Journal of
//!     Systems and Software 195, 2023, 111515.
//!     <https://doi.org/10.1016/j.jss.2022.111515>
//! 16. T. Filó, M. Bigonha, K. Ferreira. *A Catalogue of Thresholds for
//!     Object-Oriented Software Metrics.* SOFTENG 2015, 48–55.
//!     <https://personales.upv.es/thinkmind/dl/conferences/softeng/softeng_2015/softeng_2015_3_10_55070.pdf>
//!     — evaluated for bad-smell detection and fault prediction in
//!     T. Filó, M. Bigonha, K. Ferreira, *Evaluating Thresholds for
//!     Object-Oriented Software Metrics*, JBCS, 2024.
//!     <https://journals-sol.sbc.org.br/index.php/jbcs/article/view/3373>
//! 17. K. Ferreira, M. Bigonha, R. Bigonha, L. Mendes, H. Almeida.
//!     *Identifying thresholds for object-oriented software metrics.*
//!     Journal of Systems and Software 85(2), 2012, 244–257.
//!     <https://doi.org/10.1016/j.jss.2011.05.044>
//!
//! [1]: https://doi.org/10.1145/3194164.3194186
//! [2]: https://doi.org/10.1145/3382494.3410636
//! [3]: https://doi.org/10.1109/TSE.1976.233837
//! [4]: https://doi.org/10.1002/smr.1760
//! [5]: https://doi.org/10.1109/QUATIC.2007.7
//! [6]: https://doi.org/10.1109/ICSM.2010.5609747
//! [7]: https://doi.org/10.1037/h0043158
//! [8]: https://doi.org/10.1017/S0140525X01003922
//! [9]: https://doi.org/10.1109/ICSE43902.2021.00056
//! [10]: https://en.wikipedia.org/wiki/Halstead_complexity_measures
//! [11]: https://etd.auburn.edu/handle/10415/8100
//! [12]: https://avandeursen.com/2014/08/29/think-twice-before-using-the-maintainability-index/
//! [13]: https://doi.org/10.1109/32.935855
//! [14]: https://doi.org/10.1145/2372251.2372269
//! [15]: https://doi.org/10.1016/j.jss.2022.111515
//! [16]: https://personales.upv.es/thinkmind/dl/conferences/softeng/softeng_2015/softeng_2015_3_10_55070.pdf
//! [17]: https://doi.org/10.1016/j.jss.2011.05.044

mod config;
mod score;

pub use config::{HealthConfig, PILLARS, Weights};
pub use score::{
    ClassHealth, ClassScores, FileHealth, FunctionHealth, Grade, Measure, Pillar, RepoHealth,
    Scores, file_health,
};
