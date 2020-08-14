//! Error types and error reporting.
//!
//! Define error types for different phases of the execution, together with functions to generate a
//! [codespan](https://crates.io/crates/codespan-reporting) diagnostic from them.
use crate::eval::CallStack;
use crate::identifier::Ident;
use crate::label;
use crate::position::RawSpan;
use crate::term::RichTerm;
use codespan::{FileId, Files};
use codespan_reporting::diagnostic::{Diagnostic, Label, LabelStyle};

/// A general error occurring during either parsing or evaluation.
#[derive(Debug, PartialEq)]
pub enum Error {
    EvalError(EvalError),
    ParseError(String),
}

/// An error occurring during evaluation.
#[derive(Debug, PartialEq)]
pub enum EvalError {
    /// A blame occurred: a contract have been broken somewhere.
    BlameError(label::Label, Option<CallStack>),
    /// Mismatch between the expected type and the actual type of an expression.
    TypeError(
        /* expected type */ String,
        /* operation */ String,
        RichTerm,
    ),
    /// A term which is not a function has been applied to an argument.
    NotAFunc(
        /* term */ RichTerm,
        /* arg */ RichTerm,
        /* app position */ Option<RawSpan>,
    ),
    /// A field access, or another record operation requiring the existence of a specific field,
    /// has been performed on a record missing that field.
    FieldMissing(
        /* field identifier */ String,
        /* operator */ String,
        RichTerm,
        Option<RawSpan>,
    ),
    /// Too few arguments were provided to a builtin function.
    NotEnoughArgs(
        /* required arg count */ usize,
        /* primitive */ String,
        Option<RawSpan>,
    ),
    /// Attempted to merge incompatible values: for example, tried to merge two distinct default
    /// values into one record field.
    MergeIncompatibleArgs(
        /* left operand */ RichTerm,
        /* right operand */ RichTerm,
        /* original merge */ Option<RawSpan>,
    ),
    /// An unbound identifier was referenced.
    UnboundIdentifier(Ident, Option<RawSpan>),
    /// Errors occurring rarely enough to not deserve a dedicated variant.
    Other(String, Option<RawSpan>),
}

impl From<EvalError> for Error {
    fn from(error: EvalError) -> Error {
        Error::EvalError(error)
    }
}

/// A trait for converting an error to a diagnostic.
pub trait ToDiagnostic<FileId> {
    /// Convert an error to a printable, formatted diagnostic.
    ///
    /// To know why it takes a mutable reference to `Files<String>`, see [`label_alt`](fn.label_alt.html).
    fn to_diagnostic(&self, files: &mut Files<String>) -> Diagnostic<FileId>;
}

// Helpers for the creation of codespan `Label`s

/// Create a primary label from a span.
fn primary(span: &RawSpan) -> Label<FileId> {
    Label::primary(span.src_id, span.start.to_usize()..span.end.to_usize())
}

/// Create a secondary label from a span.
fn secondary(span: &RawSpan) -> Label<FileId> {
    Label::secondary(span.src_id, span.start.to_usize()..span.end.to_usize())
}

/// Create a label from an optional span, or fallback to annotating the alternative snippet
/// `alt_term` if the span is `None`.
///
/// When `span_opt` is `None`, the code snippet `alt_term` is added to `files` under a special
/// name and is referred to instead.
///
/// This is useful because during evaluation, some terms are the results of computations. They
/// correspond to nothing in the original source, and thus have a position set to `None`(e.g. the
/// result of `let x = 1 + 1 in x`).  In such cases it may still be valuable to print the term (or
/// a terse representation) in the error diagnostic rather than nothing, because if you have let `x
/// = 1 + 1 in` and then 100 lines later, `x arg` - causing an `NotAFunc` error - it may be helpful
/// to know that `x` holds the value `2`.
///
/// For example, if one wants to report an error on a record, `alt_term` may be defined to `{ ...  }`.
/// Then, if this record has no position (`span_opt` is `None`), the error will be reported as:
///
/// ```
/// error: some error
///   -- <unkown> (generated by evaluation):1:2
///   |
/// 1 | { ... }
///     ^^^^^^^ some annotation
/// ```
///
/// The reason for the mutable reference to `files` is that codespan do no let you annotate
/// something that is not in `files`: you can't provide a raw snippet, you need to provide a
/// `FileId` referring to a file. This leaves the following possibilities:
///
/// 1. Do nothing: just elude annotations which refer to the term
/// 2. Print the term and the annotation as a note together with the diagnostic. Notes are
///    additional text placed at the end of diagnostic. What you lose:
///     - pretty formatting of annotations for such snippets
///     - style consistency: the style of the error now depends on the term being from the source
///     or a byproduct of evaluation
/// 3. Add the term to files, take 1: pass a reference to files so that the code building the
///    diagnostic can itself add arbitrary snippets if necessary, and get back their `FileId`. This
///    is what is done here.
/// 4. Add the term to files, take 2: make a wrapper around the `Files` and `FileId` structures of
///    codespan which handle source mapping. `FileId` could be something like
///    `Either<codespan::FileId, CustomId = u32>` so that `to_diagnostic` could construct and use these
///    separate ids, and return the corresponding snippets to be added together with the
///    diagnostic without modifying external state. Or even have `FileId = Either<codespan::FileId`,
///    `LoneCode = String or (Id, String)>` so we don't have to return the additional list of
///    snippets. This adds some boilerplate, that we wanted to avoid, but this stays on the
///    reasonable side of being an alternative.
fn label_alt(
    span_opt: &Option<RawSpan>,
    alt_term: String,
    style: LabelStyle,
    files: &mut Files<String>,
) -> Label<FileId> {
    match span_opt {
        Some(span) => Label::new(
            style,
            span.src_id,
            span.start.to_usize()..span.end.to_usize(),
        ),
        None => {
            let range = 1..alt_term.len();
            Label::new(
                style,
                files.add("<unkown> (generated by evaluation)", alt_term),
                range,
            )
        }
    }
}

/// Create a secondary label from an optional span, or fallback to annotating the alternative snippet
/// `alt_term` if the span is `None`.
///
/// See [`label_alt`](fn.label_alt.html).
fn primary_alt(
    span_opt: &Option<RawSpan>,
    alt_term: String,
    files: &mut Files<String>,
) -> Label<FileId> {
    label_alt(span_opt, alt_term, LabelStyle::Primary, files)
}

/// Create a primary label from a term, or fallback to annotating the shallow representation of this term
/// if its span is `None`.
///
/// See [`label_alt`](fn.label_alt.html).
fn primary_term(term: &RichTerm, files: &mut Files<String>) -> Label<FileId> {
    primary_alt(&term.pos, term.as_ref().shallow_repr(), files)
}

/// Create a secondary label from an optional span, or fallback to annotating the alternative snippet
/// `alt_term` if the span is `None`.
///
/// See [`label_alt`](fn.label_alt.html).
fn secondary_alt(
    span_opt: &Option<RawSpan>,
    alt_term: String,
    files: &mut Files<String>,
) -> Label<FileId> {
    label_alt(span_opt, alt_term, LabelStyle::Secondary, files)
}

impl ToDiagnostic<FileId> for Error {
    fn to_diagnostic(&self, files: &mut Files<String>) -> Diagnostic<FileId> {
        match self {
            Error::ParseError(msg) => {
                Diagnostic::error().with_message(format!("While parsing: {}", msg.clone()))
            }
            Error::EvalError(err) => err.to_diagnostic(files),
        }
    }
}

impl ToDiagnostic<FileId> for EvalError {
    fn to_diagnostic(&self, files: &mut Files<String>) -> Diagnostic<FileId> {
        match self {
            EvalError::BlameError(l, _cs_opt) => {
                let mut msg = format!("Blame error: [{}].", &l.tag);

                if l.polarity {
                    msg.push_str("  The blame is on the value (positive blame)\n");
                } else {
                    msg.push_str("  The blame is on the context (negative blame)\n");
                }

                if l.path != label::TyPath::Nil() {
                    msg.push_str(&format!("{:?}", l.path));
                }

                Diagnostic::error()
                    .with_message(msg)
                    .with_labels(vec![Label::primary(
                        l.span.src_id,
                        l.span.start.to_usize()..l.span.end.to_usize(),
                    )
                    .with_message("bound here")])
            }
            EvalError::TypeError(expd, msg, t) => {
                let label = format!(
                    "This expression has type {}, but {} was expected",
                    t.term.type_of().unwrap_or(String::from("<unevaluated>")),
                    expd,
                );

                Diagnostic::error()
                    .with_message("Type error")
                    .with_labels(vec![primary_term(&t, files).with_message(label)])
                    .with_notes(vec![msg.clone()])
            }
            EvalError::NotAFunc(t, arg, pos_opt) => Diagnostic::error()
                .with_message("Not a function")
                .with_labels(vec![
                    primary_term(&t, files)
                        .with_message("this term is applied, but it is not a function"),
                    secondary_alt(
                        &pos_opt,
                        format!(
                            "({}) ({})",
                            (*t.term).shallow_repr(),
                            (*arg.term).shallow_repr()
                        ),
                        files,
                    )
                    .with_message("applied here"),
                ]),
            EvalError::FieldMissing(field, op, t, span_opt) => {
                let mut labels = Vec::new();
                let mut notes = Vec::new();

                if let Some(span) = span_opt {
                    labels.push(
                        Label::primary(span.src_id, span.start.to_usize()..span.end.to_usize())
                            .with_message(format!("this requires field {} to exist", field)),
                    );
                } else {
                    notes.push(format!(
                        "Field {} was required by the operator {}",
                        field, op
                    ));
                }

                if let Some(ref span) = t.pos {
                    labels.push(
                        secondary(span).with_message(format!("field {} is missing here", field)),
                    );
                }

                Diagnostic::error()
                    .with_message("Missing field")
                    .with_labels(labels)
            }
            EvalError::NotEnoughArgs(count, op, span_opt) => {
                let mut labels = Vec::new();
                let mut notes = Vec::new();
                let msg = format!(
                    "{} expects {} arguments, but not enough were provided",
                    op, count
                );

                if let Some(span) = span_opt {
                    labels.push(
                        Label::primary(span.src_id, span.start.to_usize()..span.end.to_usize())
                            .with_message(msg),
                    );
                } else {
                    notes.push(msg);
                }

                Diagnostic::error()
                    .with_message("Not enough arguments")
                    .with_labels(labels)
                    .with_notes(notes)
            }
            EvalError::MergeIncompatibleArgs(t1, t2, span_opt) => {
                let mut labels = vec![
                    primary_term(&t1, files).with_message("cannot merge this expression"),
                    primary_term(&t2, files).with_message("with this expression"),
                ];

                if let Some(span) = span_opt {
                    labels.push(secondary(&span).with_message("merged here"));
                }

                Diagnostic::error()
                    .with_message("Non mergeable terms")
                    .with_labels(labels)
            }
            EvalError::UnboundIdentifier(Ident(ident), span_opt) => Diagnostic::error()
                .with_message("Unbound identifier")
                .with_labels(vec![primary_alt(span_opt, String::from(ident), files)
                    .with_message("this identifier is unbound")]),
            EvalError::Other(msg, span_opt) => {
                let labels = span_opt
                    .as_ref()
                    .map(|span| vec![primary(span).with_message("here")])
                    .unwrap_or(Vec::new());

                Diagnostic::error().with_message(msg).with_labels(labels)
            }
        }
    }
}
