//! An interactive command line interface for running a tournament with [`tournament_engine`].
//!
//! ```text
//! tournament-cli [FILE]
//! ```
//!
//! Loads the competition saved to `FILE` if it exists, and otherwise starts a new one that is
//! saved there. Type `help` at the prompt for the available commands.

use std::{
    io::{self, IsTerminal},
    path::Path,
    process::ExitCode,
};

use clap::Parser;
use inquire::InquireError;
use plugin_factory::draft::Draft;
use rustyline::{DefaultEditor, error::ReadlineError};
use thiserror::Error;
use tournament_cli::{
    command::{Command, ParseError},
    questionnaire::{self, Terminal},
    session::{self, Configuring, Outcome, Session},
    storage::FileStorage,
};
use tournament_engine::PhasePlugins;

/// Runs a tournament interactively.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// The file the competition is loaded from, if it exists, and saved to
    file: Option<String>,
}

/// Why the session ended other than by an `exit`.
#[derive(Debug, Error)]
enum Error {
    #[error(transparent)]
    Session(#[from] session::Error),
    #[error("cannot configure the phase: {0}")]
    Configure(questionnaire::Error),
    #[error("cannot read from the terminal: {0}")]
    Terminal(#[from] ReadlineError),
    #[error("cannot write to the terminal: {0}")]
    Output(#[from] io::Error),
    #[error("the input ended with unsaved changes, which are discarded")]
    InputEnded,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let session = match args.file {
        Some(file) if Path::new(&file).exists() => Session::load(FileStorage, file),
        Some(file) => Ok(Session::new(FileStorage).named(file)),
        None => Ok(Session::new(FileStorage)),
    };
    match session.map_err(Error::from).and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Reads and executes commands until one ends the session.
fn run(mut session: Session) -> Result<(), Error> {
    let mut editor = DefaultEditor::new()?;
    loop {
        let command = match editor.readline(&prompt(&session)) {
            Ok(line) => {
                editor.add_history_entry(line.as_str())?;
                match Command::parse_line(&line) {
                    Ok(Some(command)) => command,
                    Ok(None) => continue,
                    Err(ParseError::Clap(error)) => {
                        // also shows help, which clap reports as an error
                        error.print()?;
                        continue;
                    }
                    Err(error) => {
                        eprintln!("error: {error}");
                        continue;
                    }
                }
            }
            // Ctrl-C only discards the line being typed
            Err(ReadlineError::Interrupted) => continue,
            // Ctrl-D exits, but not past unsaved changes
            Err(ReadlineError::Eof) if io::stdin().is_terminal() => Command::Exit { force: false },
            // input that is not a terminal ends for good, and every further read would end the
            // same way, so unsaved changes can neither be saved nor kept
            Err(ReadlineError::Eof) if session.has_unsaved_changes() => {
                return Err(Error::InputEnded);
            }
            Err(ReadlineError::Eof) => return Ok(()),
            Err(error) => return Err(Error::Terminal(error)),
        };
        // a session never asks the operator anything, so a phase that is to be configured is
        // configured here and reaches the session as the configuration it produced
        let outcome = match command {
            Command::AddPhase {
                format: None,
                name,
                from,
                ..
            } => session
                .new_phase_draft(&from)
                .map_err(Error::Session)
                .and_then(|draft| {
                    configure_phase(&mut session, draft, &Configuring::Added { name, from })
                }),
            Command::EditPhase { phase } => session
                .phase_draft_to_edit(phase)
                .map_err(Error::Session)
                .and_then(|draft| {
                    configure_phase(&mut session, draft, &Configuring::Edited { phase })
                }),
            command => session.execute(command).map_err(Error::Session),
        };
        match outcome {
            Ok(Outcome::Continue(output)) if output.is_empty() => {}
            Ok(Outcome::Continue(output)) => println!("{output}"),
            Ok(Outcome::Exit) => return Ok(()),
            Err(error) => eprintln!("error: {error}"),
        }
    }
}

/// Configures a phase at the prompt, starting out at the answers of `draft`, and hands it to the
/// session as `configuring` says, leaving the competition untouched if the operator cancels the
/// configuration. A phase whose plugins do not fit together is configured again, starting out at
/// the answers given.
fn configure_phase(
    session: &mut Session,
    mut draft: Draft<PhasePlugins>,
    configuring: &Configuring,
) -> Result<Outcome, Error> {
    loop {
        match questionnaire::fill_phase(&mut draft, &mut Terminal) {
            Ok(()) => {}
            Err(questionnaire::Error::Cancelled) => {
                return Ok(Outcome::Continue(configuring.cancelled().to_owned()));
            }
            // input that is piped in cannot answer any question, which is what the session says
            // when it is handed the command itself
            Err(questionnaire::Error::Prompt(InquireError::NotTTY)) => {
                return Err(Error::Session(match configuring {
                    Configuring::Added { .. } => session::Error::NotConfigured,
                    Configuring::Edited { .. } => session::Error::NotEdited,
                }));
            }
            Err(error) => return Err(Error::Configure(error)),
        }
        match session.apply_phase(configuring, &draft) {
            Err(error @ session::Error::InvalidPhase(_)) => {
                eprintln!("error: {error}\nplease answer again");
            }
            applied => return applied.map(Outcome::Continue).map_err(Error::Session),
        }
    }
}

/// Names the file the competition is saved to and marks unsaved changes with a `*`.
fn prompt(session: &Session) -> String {
    let name = session
        .name()
        .and_then(|file| Path::new(file).file_stem())
        .map_or("tournament".into(), |stem| stem.to_string_lossy());
    let unsaved = if session.has_unsaved_changes() {
        "*"
    } else {
        ""
    };
    format!("{name}{unsaved}> ")
}
