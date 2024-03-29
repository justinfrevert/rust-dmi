use factorial::Factorial;
use ndarray::{prelude::*, ViewRepr};
use ndarray_linalg::{error::LinalgError, solve::Determinant};

#[derive(Debug, PartialEq)]
pub enum DMIError {
    Arithmetic,
    /// The values of answers must be integers in [0, C)
    AnswerValsOutOfScope,
    // More specific error for differentiating different errors internally
    FactorialCalc,
    FactorialMulCalc,
    Exponentiate,
    LinalgError,
    NLessThanM,
    PaymentNLessThanM,
    // At least one agent should have been engaged
    TooFewAgents,
    /// Tasks must be greater than or equal to twice the choice count
    TooFewTasks,
    /// Only one agent or fewer given when calculating payments
    TooFewAgentsForPaymentCalc,
    PaymentFactorialCalc,
}

impl From<LinalgError> for DMIError {
    fn from(_: LinalgError) -> DMIError {
        DMIError::LinalgError
    }
}

pub trait DMI {
    // factorial(n) / (factorial(m) * factorial(n - m))
    fn calculate_factorials(n: &usize, m: &usize) -> Result<f32, DMIError> {
        let factorial_n = Factorial::checked_factorial(n).ok_or(DMIError::FactorialCalc)?;
        let factorial_mul_result = {
            let factorial_m = Factorial::checked_factorial(m).ok_or(DMIError::FactorialCalc)?;

            let factorial_n_minus_m =
                Factorial::checked_factorial(&(n.checked_sub(*m).ok_or(DMIError::NLessThanM)?))
                    .ok_or(DMIError::FactorialCalc)?;

            factorial_m
                .checked_mul(factorial_n_minus_m)
                .ok_or(DMIError::FactorialMulCalc)?
        };

        Ok(factorial_n
            .checked_div(factorial_mul_result)
            .ok_or(DMIError::Arithmetic)? as f32)
    }

    fn check_answers(x: &usize, c: &usize) -> bool {
        &0 <= x && x < c
    }

    // get M mechanism
    // a and b are equal length
    fn get_mechanism<'a>(
        a: ArrayView1<usize>,
        b: ArrayView1<usize>,
        c: &usize,
    ) -> Result<Array2<f32>, DMIError> {
        let mut mechanism = Array2::<f32>::zeros((*c, *c));
        for (x, y) in a.into_iter().zip(b.into_iter()) {
            if Self::check_answers(&x, &c) && Self::check_answers(&y, &c) {
                if let Some(v) = mechanism.get_mut((*x, *y)) {
                    *v += 1.;
                }
            } else {
                return Err(DMIError::AnswerValsOutOfScope);
            }
        }
        Ok(mechanism)
    }

    // aka dmi2 in source
    // DMI(X; Y ) = ∣ det(UX,Y )∣.
    fn get_mutual_information(
        a1: ArrayView1<usize>,
        b1: ArrayView1<usize>,
        a2: ArrayView1<usize>,
        b2: ArrayView1<usize>,
        c: &usize,
    ) -> Result<f32, DMIError> {
        let m1 = Self::get_mechanism(a1, b1, c)?;
        let m2 = Self::get_mechanism(a2, b2, c)?;
        Ok(m1.det()? * m2.det()?)
    }

    // Do the actual DMI calculation
    // Note: the size of the returned vector is predictably larger than the same calculation done in the python version
    fn do_dmi(answers: Array2<usize>, choice_n: usize) -> Result<Vec<f32>, DMIError> {
        let answers_shape = answers.shape();
        let agent_n = answers_shape[0];
        let task_n = answers_shape[1];

        // T >= 2C; N > 1;
        if task_n < 2 * choice_n {
            return Err(DMIError::TooFewTasks);
        }
        if agent_n <= 1 {
            return Err(DMIError::TooFewAgents);
        }

        // Arbitrarily split answers
        let transposed = answers.t();
        let view = ArrayView2::from(transposed);
        // TODO: shuffle all answers here
        // get half and split array at it
        let half = task_n / 2;
        // t1, and t2
        let (first_half_answers, second_half_answers) = view.split_at(Axis(0), half);

        // transpose both
        let first_half_answers = first_half_answers.t();
        let second_half_answers = second_half_answers.t();

        let payments =
            Self::calculate_payments(&agent_n, &choice_n, first_half_answers, second_half_answers);

        payments
    }

    fn calculate_payments(
        agent_n: &usize,
        choice_n: &usize,
        t1: ArrayBase<ViewRepr<&usize>, Dim<[usize; 2]>>,
        t2: ArrayBase<ViewRepr<&usize>, Dim<[usize; 2]>>,
    ) -> Result<Vec<f32>, DMIError> {
        let prelim_agents = (agent_n.checked_sub(1)).ok_or(DMIError::TooFewAgentsForPaymentCalc)?;
        let fact = Factorial::checked_factorial(choice_n).ok_or(DMIError::PaymentFactorialCalc)?;
        let raised = fact.checked_pow(2).ok_or(DMIError::Exponentiate)?;

        let mut norm_factor = prelim_agents
            .checked_mul(raised)
            .ok_or(DMIError::Arithmetic)? as f32;

        norm_factor *= Self::calculate_factorials(&t1.shape()[0], choice_n)?
            * Self::calculate_factorials(&t2.shape()[0], choice_n)?;

        let mut payments = vec![];
        for i in 0..*agent_n {
            let mut p = 0_f32;
            for j in 0..*agent_n {
                if i == j {
                    continue;
                }

                p += Self::get_mutual_information(
                    t1.slice(s![i, ..,]),
                    t1.slice(s![j, ..,]),
                    t2.slice(s![i, ..,]),
                    t2.slice(s![j, ..,]),
                    choice_n,
                )
                .unwrap();

                p /= norm_factor;
                payments.push(p);
            }
        }

        Ok(payments)
    }
}
