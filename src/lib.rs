#![no_std]

use gstd::{prelude::*, exec, msg};
use pebbles_game_io::{GameError, PebblesAction, PebblesEvent, Player};
use pebbles_game_io::{DifficultyLevel, GameState, PebblesInit};

static mut GAME: Option<Game> = None;

fn state_mut() -> &'static mut Game {
    let state = unsafe { GAME.as_mut() };

    unsafe { state.unwrap_unchecked() }
}

#[derive(Clone, Default)]
struct Game {
    pebbles_count: u32,
    max_pebbles_per_turn: u32,
    pebbles_remaining: u32,
    difficulty: DifficultyLevel,
    first_player: Player,
    winner: Option<Player>,
}

impl Game {
    fn make_program_move(&mut self) -> Result<PebblesEvent, GameError> {
        if !self.winner.is_none() {
            return Err(GameError::GameAlreadyFinished);
        }

        let number_of_pebbles_removed_by_program = get_program_move(self.pebbles_remaining, self.max_pebbles_per_turn, &self.difficulty);
        self.pebbles_remaining -= number_of_pebbles_removed_by_program;
        if self.pebbles_remaining == 0 {
            self.winner = Some(Player::Program);

            return Ok(PebblesEvent::Won(Player::Program))
        }

        Ok(PebblesEvent::CounterTurn(number_of_pebbles_removed_by_program))
    }

    fn make_user_move(&mut self, number_of_pebbles_to_be_removed: u32) -> Result<PebblesEvent, GameError> {
        if !self.winner.is_none() {
            return Err(GameError::GameAlreadyFinished);
        }

        if (number_of_pebbles_to_be_removed < 1) || (number_of_pebbles_to_be_removed > self.max_pebbles_per_turn) {
            return Err(GameError::InvalidNumberOfPebblesToBeRemoved);
        }

        self.pebbles_remaining -= number_of_pebbles_to_be_removed;
        if self.pebbles_remaining == 0 {
            self.winner = Some(Player::User);
            return Ok(PebblesEvent::Won(Player::User));
        }

        self.make_program_move()
    }
}

impl From<Game> for GameState {
    fn from(game: Game) -> Self {
        GameState {
            pebbles_count: game.pebbles_count,
            max_pebbles_per_turn: game.max_pebbles_per_turn,
            pebbles_remaining: game.pebbles_remaining,
            difficulty: game.difficulty,
            first_player: game.first_player,
            winner: game.winner,
        }
    }
}

#[no_mangle]
extern "C" fn init() {
    let init_msg: PebblesInit = msg::load().expect("Unable to load the initial message");

    // Validate the initial message
    validate_init_msg(&init_msg).expect("Invalid initial message");

    let first_player = if get_random_u32() % 2 == 0 {
        Player::User
    } else {
        Player::Program
    };

    // Initialize the game
    let mut game = Game {
        pebbles_count: init_msg.pebbles_count,
        max_pebbles_per_turn: init_msg.max_pebbles_per_turn,
        pebbles_remaining: init_msg.pebbles_count,
        difficulty: init_msg.difficulty,
        first_player,
        winner: None,
    };

    if let Player::Program = game.first_player.clone() {
        game.make_program_move().expect("Program unable to make a move");
    }

    msg::reply::<GameState>(game.clone().into(), 0)
        .expect("Failed to reply with `Game initialized` from `init()`");

    unsafe {
        GAME = Some(game);
    }
}

#[no_mangle]
extern fn handle() {
    let game = unsafe { GAME.as_mut().expect("`Game` is not initialized") };
    let action: PebblesAction = msg::load().expect("Failed to decode `PebblesAction` message.");

    let reply = match action {
        PebblesAction::Turn(num) => game.make_user_move(num).expect("User unable to make a move"),
        PebblesAction::GiveUp => {
            game.winner = Some(Player::Program);

            PebblesEvent::Won(Player::Program)
        },
        PebblesAction::Restart {
            difficulty,
            pebbles_count,
            max_pebbles_per_turn
        } => {
            validate_init_msg(&PebblesInit {
                difficulty: difficulty.clone(),
                pebbles_count,
                max_pebbles_per_turn,
            }).expect("Invalid restart message");

            game.pebbles_count = pebbles_count;
            game.max_pebbles_per_turn = max_pebbles_per_turn;
            game.pebbles_remaining = pebbles_count;
            game.difficulty = difficulty;
            game.first_player = if get_random_u32() % 2 == 0 {
                Player::User
            } else {
                Player::Program
            };
            game.winner = None;

            let mut counter_turn = 0;
            if let Player::Program = game.first_player {
                match game.make_program_move() {
                    Ok(event) => {
                        counter_turn = match event {
                            PebblesEvent::CounterTurn(num) => num,
                            _ => 0,
                        };
                    },
                    _ => ()
                }
            }

            PebblesEvent::CounterTurn(counter_turn)
        }

    };

    msg::reply::<PebblesEvent>(reply, 0)
        .expect("Failed to reply with `PebblesEvent` from `main()`");
}

#[no_mangle]
extern "C" fn state() {
    let state = unsafe { GAME.take().expect("Unexpected error while taking state.") };
    msg::reply::<GameState>(state.into(), 0)
        .expect("Failed to reply with `Game state` from `state()`");
}

fn validate_init_msg(init_msg: &PebblesInit) -> Result<(), GameError> {
    if init_msg.pebbles_count < 2 {
        return Err(GameError::AtLeastTwoPebblesToStart);
    }

    if init_msg.max_pebbles_per_turn < 1 {
        return Err(GameError::AtLeastOnePebblePerTurnToStart);
    }

    if init_msg.max_pebbles_per_turn >= init_msg.pebbles_count {
        return Err(GameError::InvalidNumberOfPebblesToBeRemoved);
    }

    Ok(())
}

fn get_program_move(current_pebbles: u32, max_remove: u32, difficulty: &DifficultyLevel) -> u32 {
    match difficulty {
        DifficultyLevel::Easy => get_random_u32() % max_remove.min(current_pebbles) + 1,
        DifficultyLevel::Hard => get_winning_move(current_pebbles, max_remove),
    }
}

fn get_winning_move(current_pebbles: u32, max_remove: u32) -> u32 {
    let target = (current_pebbles - 1) % (max_remove + 1);
    if target == 0 {
        max_remove.min(current_pebbles)
    } else {
        target
    }
}

fn get_random_u32() -> u32 {
    let salt = msg::id();
    let (hash, _num) = exec::random(salt.into()).expect("get_random_u32(): random call failed");

    u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]])
}
