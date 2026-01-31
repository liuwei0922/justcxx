use justcxx::bind;

bind! {
    include!("test.hh");
    struct Config{
        id: i32,
        value: f32,
        name: String,
    }

    struct Manager{
        config: Config,
    }

    struct School{
        teacher: Manager,
        student: Manager,
        other: Manager,
    }

    struct Methods{
        #[readonly]
        id: i32,
        #[readonly]
        config: Config,
    }

    struct Chance{
        probability: Option<i32>,
    }

    struct Wallet{
        #[readonly]
        config: Option<Config>,
    }

    struct MapExample{
        int_str_map: Map<i32, String>,
        int_config_map: Map<i32, Config>,
        str_config_map: Map<String, Config>
    }

    struct ConfigContainer{
        data: Vec<Config>,
        ids: Vec<i32>,
        names: Vec<String>,
    }

    impl Methods{
        fn get_id(&self)-> i32;

        fn set_id(&mut self,v: i32);

        fn get_config(&self) -> &Config;

        fn set_config(&mut self, c: &Config);

        fn set_config_by_value(&mut self, c: Config);

        fn add(v: i32, w: i32) -> i32;

        fn create_config(&self) -> Config;

        fn optional_id(&self, flag: bool) -> Option<i32>;

        fn return_nums(&mut self) -> &mut Vec<i32>;
    }

    impl ConfigContainer{
        #[iter(Item = Config)]
        fn drain(&mut self);
    }

    impl Chance{
        fn set_chance(&mut self, flag: bool, p: i32);
    }

    impl Wallet{
        fn set_config(&mut self, flag: bool, c: &Config);
    }

    struct Ctor{
        id:i32,
    }

}

pub mod test;
