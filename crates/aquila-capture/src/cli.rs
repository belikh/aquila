//! A tiny, dependency-free command-line parser.
//!
//! The tool deliberately pulls in no argument-parsing crate: flags are
//! `--name value` or `--name=value`, and the accepted set is declared per
//! subcommand by [`Flag`] values. Parsing is total and never panics.

use std::collections::BTreeMap;

use crate::error::CaptureError;

/// The declaration of one accepted flag.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Flag {
    /// The flag name, without the leading `--`.
    pub(crate) name: &'static str,
    /// A one-line description shown by `--help`.
    pub(crate) help: &'static str,
    /// Whether the flag must be present.
    required: bool,
}

impl Flag {
    /// A flag that must be supplied.
    pub(crate) const fn required(name: &'static str, help: &'static str) -> Self {
        Self {
            name,
            help,
            required: true,
        }
    }

    /// A flag that may be omitted.
    pub(crate) const fn optional(name: &'static str, help: &'static str) -> Self {
        Self {
            name,
            help,
            required: false,
        }
    }
}

/// A parsed command line: named flags plus any positional arguments.
#[derive(Debug, Clone)]
pub(crate) struct Options {
    flags: BTreeMap<String, String>,
    positional: Vec<String>,
}

impl Options {
    /// Parse `args` against the accepted `flags`.
    ///
    /// Returns `Ok(None)` when the caller asked for `--help`; the help text is
    /// printed as a side effect.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Usage`] for an unknown flag, a missing value or a
    /// missing required flag.
    pub(crate) fn parse(
        args: &[String],
        flags: &[Flag],
        command: &str,
    ) -> Result<Option<Self>, CaptureError> {
        let mut parsed: BTreeMap<String, String> = BTreeMap::new();
        let mut positional: Vec<String> = Vec::new();
        let mut index = 0usize;
        while index < args.len() {
            let arg = &args[index];
            if arg == "--help" || arg == "-h" {
                print_help(command, flags);
                return Ok(None);
            }
            if let Some(rest) = arg.strip_prefix("--") {
                let (name, value) = match rest.split_once('=') {
                    Some((name, value)) => (name, Some(value.to_string())),
                    None => (rest, None),
                };
                if !flags.iter().any(|flag| flag.name == name) {
                    return Err(CaptureError::Usage(format!(
                        "{command}: unknown flag --{name}"
                    )));
                }
                let value = match value {
                    Some(value) => value,
                    None => {
                        index += 1;
                        args.get(index).cloned().ok_or_else(|| {
                            CaptureError::Usage(format!("{command}: flag --{name} needs a value"))
                        })?
                    }
                };
                parsed.insert(name.to_string(), value);
            } else {
                positional.push(arg.clone());
            }
            index += 1;
        }

        for flag in flags {
            if flag.required && !parsed.contains_key(flag.name) {
                return Err(CaptureError::Usage(format!(
                    "{command}: missing required flag --{}",
                    flag.name
                )));
            }
        }

        Ok(Some(Self {
            flags: parsed,
            positional,
        }))
    }

    /// The value of `name`, when present and non-empty.
    pub(crate) fn get(&self, name: &str) -> Option<&str> {
        self.flags.get(name).map(String::as_str)
    }

    /// The value of `name`, or `default` when absent.
    pub(crate) fn get_or<'a>(&'a self, name: &str, default: &'a str) -> &'a str {
        self.get(name).unwrap_or(default)
    }

    /// Parse an optional flag as a `u16`, falling back to `default`.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Usage`] when the flag is present but not a
    /// `u16`.
    pub(crate) fn get_u16_or(&self, name: &str, default: u16) -> Result<u16, CaptureError> {
        match self.get(name) {
            None => Ok(default),
            Some(value) => value
                .parse::<u16>()
                .map_err(|_| CaptureError::Usage(format!("--{name} must be a u16, got {value:?}"))),
        }
    }

    /// Any positional arguments, in order.
    pub(crate) fn positional(&self) -> &[String] {
        &self.positional
    }

    /// Dispatch to the `record` subcommand.
    pub(crate) fn run_record(&self) -> Result<(), CaptureError> {
        crate::record::run(self)
    }

    /// Dispatch to the `synth` subcommand.
    pub(crate) fn run_synth(&self) -> Result<(), CaptureError> {
        crate::synth::run(self)
    }

    /// Dispatch to the `verify` subcommand.
    pub(crate) fn run_verify(&self) -> Result<(), CaptureError> {
        crate::verify::run(self)
    }

    /// Dispatch to the `provision` subcommand.
    pub(crate) fn run_provision(&self) -> Result<(), CaptureError> {
        crate::provision::run(self)
    }
}

/// Print a subcommand's flag help.
fn print_help(command: &str, flags: &[Flag]) {
    println!("aquila-capture {command} — flags:");
    for flag in flags {
        let marker = if flag.required {
            "(required)"
        } else {
            "(optional)"
        };
        println!("  --{:<18} {marker:<10} {}", flag.name, flag.help);
    }
}
