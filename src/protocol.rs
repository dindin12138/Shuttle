pub mod river_window_manager {
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/river-window-management-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocols/river-window-management-v1.xml");
}

pub mod river_xkb_bindings {
    use super::river_window_manager::river_seat_v1;
    use wayland_client;

    pub mod __interfaces {
        use super::super::river_window_manager::__interfaces::*;

        wayland_scanner::generate_interfaces!("protocols/river-xkb-bindings-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocols/river-xkb-bindings-v1.xml");
}
