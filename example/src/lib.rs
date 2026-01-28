use justcxx::bind;

bind! {
    include!("test.hh");
    struct Config{
        id: i32,
        value: f32,
        name: String,
    }

    impl Config{
        fn new()-> Self;
    }

    struct Manager{
        config: Config,
    }

    impl Manager{
        fn new() -> Self;
    }

    struct School{
        teacher: Manager,
        student: Manager,
        other: Manager,
    }
    impl School{
        fn new()-> Self;
    }

    struct Methods{
        #[readonly]
        id: i32,
        #[readonly]
        config: Config,
    }

    impl Methods{
        fn new() -> Self;

        fn get_id(&self)-> i32;

        fn set_id(&mut self,v: i32);

        fn get_config(&self) -> &Config;

        fn set_config(&mut self, c: &Config);

        fn set_config_by_value(&mut self, c: Config);

        fn add(v: i32, w: i32) -> i32;

        fn create_config(&self) -> Config;

        fn optional_id(&self, flag: bool) -> Option<i32>;
    }

    struct ConfigContainer{
        data: Vec<Config>,
        names: Vec<String>,
        ids: Vec<i32>,
    }

    impl ConfigContainer{
        fn new() -> Self;
        #[iter(Item = Config)]
        fn drain(&mut self);
    }

    struct Chance{
        probability: Option<i32>,
    }

    impl Chance{
        fn new() -> Self;
        fn set_chance(&mut self, flag: bool, p: i32);
    }

    struct Wallet{
        config: Option<Config>,
    }

    impl Wallet{
        fn new() -> Self;
        fn set_config(&mut self, flag: bool, c: &Config);
    }

    struct MapExample{
        int_str_map: Map<i32, String>,
        int_config_map: Map<i32, Config>,
        str_config_map: Map<String, Config>
    }

    impl MapExample{
        fn new() -> Self;
    }

    struct Ctor{
        id:i32,
    }
  
}



pub mod test;
