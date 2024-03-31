use autocxx::WithinUniquePtr;
use rocketsim_rs::{
    bytes::{FromBytes, ToBytes},
    cxx::UniquePtr,
    math::Vec3,
    sim::{Arena, ArenaMemWeightMode, BallState, CarConfig, CarControls, GameMode, Team},
    GameState,
};
use std::{
    io,
    net::{IpAddr, SocketAddr, UdpSocket},
    str::FromStr,
    sync::mpsc::{channel, Receiver},
    thread::sleep,
    time::{Duration, Instant},
};

// Pass this into rlviser as the first argument
// default: 45243
const RLVISER_PORT: u16 = 45243;

// Pass this into rlviser as the second argument
// default: 34254
const ROCKETSIM_PORT: u16 = 34254;

#[repr(u8)]
enum UdpPacketTypes {
    Quit,
    GameState,
}

fn ctrl_channel() -> Result<Receiver<()>, ctrlc::Error> {
    let (sender, receiver) = channel();

    // Setup Ctrl+C handler
    ctrlc::set_handler(move || {
        // Send a signal to the main thread to break the loop
        // If we can't send the signal for some reason,
        // then panic the process to shut down
        sender.send(()).unwrap();
    })?;

    Ok(receiver)
}

fn main() -> io::Result<()> {
    let socket = UdpSocket::bind(("0.0.0.0", ROCKETSIM_PORT))?;
    // print the socket address
    println!("Listening on {}", socket.local_addr()?);

    // Load rocketsim
    rocketsim_rs::init(None);

    let mut args = std::env::args();
    let _ = args.next();
    let arena_type = match args.next().as_deref() {
        Some("hoops") => GameMode::HOOPS,
        _ => GameMode::SOCCAR,
    };

    let speed = args.next().and_then(|f| f.parse().ok()).unwrap_or(1.);

    run_socket(socket, arena_type, speed)
}

fn run_socket(socket: UdpSocket, arena_type: GameMode, speed: f32) -> io::Result<()> {
    let rlviser_addr = SocketAddr::new(IpAddr::from_str("0.0.0.0").unwrap(), RLVISER_PORT);

    println!("\nPress enter to start...");
    io::stdin().read_line(&mut String::new())?;

    // We now don't want to wait for anything UDP so set to non-blocking
    socket.set_nonblocking(true)?;

    let mut arena = setup_arena(arena_type);

    // listen for Ctrl+C signal
    let break_signal = ctrl_channel().unwrap();

    // we only want to loop at 120hz
    // speed 0.5 = half speed
    // speed 2 = double speed
    let interval = Duration::from_secs_f32(1. / (120. * speed));
    let mut next_time = Instant::now() + interval;
    let mut min_state_set_buf = [0; GameState::MIN_NUM_BYTES];

    // we loop forever - can be broken by pressing Ctrl+C in terminal
    loop {
        if break_signal.try_recv().is_ok() {
            socket.send_to(&[UdpPacketTypes::Quit as u8], rlviser_addr)?;
            println!("Sent quit signal to rlviser");

            // Then break the loop
            break Ok(());
        }

        handle_return_message(&mut min_state_set_buf, &socket, &mut arena)?;

        // advance the simulation by 1 tick
        arena.pin_mut().step(1);

        // send the new game state back
        let game_state = arena.pin_mut().get_game_state();

        // Send the packet type
        socket.send_to(&[UdpPacketTypes::GameState as u8], rlviser_addr)?;
        // Then send the packet
        socket.send_to(&game_state.to_bytes(), rlviser_addr)?;

        // ensure we only calculate 120 steps per second
        let wait_time = next_time - Instant::now();
        if wait_time > Duration::default() {
            sleep(wait_time);
        }
        next_time += interval;
    }
}

fn handle_return_message(
    min_state_set_buf: &mut [u8; GameState::MIN_NUM_BYTES],
    socket: &UdpSocket,
    arena: &mut UniquePtr<Arena>,
) -> io::Result<()> {
    let mut state_set_buf = Vec::new();

    while let Ok((num_bytes, src)) = socket.peek_from(min_state_set_buf) {
        if num_bytes == 1 {
            // We got a connection and not a game state
            // So clear the byte from the socket buffer and return
            let mut buf = [0];
            socket.recv_from(&mut buf)?;

            if buf[0] == 1 {
                println!("Connection established to {src}");
            }

            continue;
        }

        // the socket sent data back
        // this is the other side telling us to update the game state
        let num_bytes = GameState::get_num_bytes(min_state_set_buf);
        state_set_buf = vec![0; num_bytes];
        socket.recv_from(&mut state_set_buf)?;
    }

    // the socket didn't send data back
    if state_set_buf.is_empty() {
        return Ok(());
    }

    // set the game state
    let game_state = GameState::from_bytes(&state_set_buf);
    if let Err(e) = arena.pin_mut().set_game_state(&game_state) {
        println!("Error setting game state: {e}");
    };

    Ok(())
}

fn setup_arena(arena_type: GameMode) -> UniquePtr<Arena> {
    let mut arena = Arena::new(arena_type, ArenaMemWeightMode::LIGHT, 120.).within_unique_ptr();

    let _ = arena.pin_mut().add_car(Team::BLUE, CarConfig::octane());
    let _ = arena.pin_mut().add_car(Team::BLUE, CarConfig::dominus());
    let _ = arena.pin_mut().add_car(Team::BLUE, CarConfig::merc());
    let _ = arena.pin_mut().add_car(Team::ORANGE, CarConfig::breakout());
    let _ = arena.pin_mut().add_car(Team::ORANGE, CarConfig::hybrid());
    let _ = arena.pin_mut().add_car(Team::ORANGE, CarConfig::plank());

    arena.pin_mut().set_ball(BallState {
        pos: Vec3::new(0., -2000., 1500.),
        vel: Vec3::new(0., 1500., 1.),
        ..Default::default()
    });

    arena.pin_mut().set_goal_scored_callback(
        |arena, _, _| {
            arena.reset_to_random_kickoff(None);
        },
        0,
    );

    arena
        .pin_mut()
        .set_all_controls(
            (1..=6u32)
                .map(|i| {
                    (
                        i,
                        CarControls {
                            throttle: 1.,
                            pitch: -0.1,
                            boost: true,
                            ..Default::default()
                        },
                    )
                })
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .unwrap();

    arena
}
