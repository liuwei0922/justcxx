#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_cpp_val_get() {
        let config = Config::new();
        let id = config.id();
        assert_eq!(id, 42);
        let value = config.value();
        assert_eq!(value, 56.0);
        let name = config.name();
        assert_eq!(name, "test");
    }

    #[test]
    fn test_cpp_val_set() {
        let mut config = Config::new();
        config.set_id(100);
        config.set_value(200.0);
        config.set_name("new_test");
        assert_eq!(config.id(), 100);
        assert_eq!(config.value(), 200.0);
        assert_eq!(config.name(), "new_test");
    }

    #[test]
    fn test_cpp_obj_get1() {
        let manager = Manager::new();
        let config = manager.as_ref().config();
        assert_eq!(config.id(), 42);
        assert_eq!(config.value(), 56.0);
        assert_eq!(config.name(), "test");
    }

    #[test]
    fn test_cpp_obj_set1() {
        let mut manager = Manager::new();
        let mut config = manager.config();
        config.set_id(100);
        config.set_value(200.0);
        config.set_name("new_test");
        assert_eq!(config.id(), 100);
        assert_eq!(config.value(), 200.0);
        assert_eq!(config.name(), "new_test");
    }
    #[test]
    fn test_cpp_obj_set2() {
        let mut manager = Manager::new();
        manager.config().set_id(150);
        manager.config().set_value(250.0);
        manager.config().set_name("another_test");
        assert_eq!(manager.config().id(), 150);
        assert_eq!(manager.config().value(), 250.0);
        assert_eq!(manager.config().name(), "another_test");
    }
    #[test]
    fn test_cpp_obj_set3() {
        let mut school = School::new();
        let mut teacher_manager = school.teacher();
        let mut student_manager = school.student();

        let mut teacher_config = teacher_manager.config();
        let mut student_config = student_manager.config();

        teacher_config.set_id(300);
        teacher_config.set_value(400.0);
        teacher_config.set_name("teacher_test");

        student_config.set_id(500);
        student_config.set_value(600.0);
        student_config.set_name("student_test");

        assert_eq!(teacher_manager.config().id(), 300);
        assert_eq!(teacher_manager.config().value(), 400.0);
        assert_eq!(teacher_manager.config().name(), "teacher_test");

        assert_eq!(student_manager.config().id(), 500);
        assert_eq!(student_manager.config().value(), 600.0);
        assert_eq!(student_manager.config().name(), "student_test");
    }
    #[test]
    fn test_cpp_obj_set4() {
        let manager = Manager::new();
        let manager_const = manager.as_ref();
        // this will panic, because const pointer has no set method.
        // manager_const.config().set_id(150);
        assert_eq!(manager_const.config().id(), 42);
        assert_eq!(manager_const.config().value(), 56.0);
        assert_eq!(manager_const.config().name(), "test");
    }
    #[test]
    fn test_cpp_obj_get2() {
        let school = School::new();
        let other_manager = school.as_ref().other();
        assert_eq!(other_manager.config().id(), 42);
        assert_eq!(other_manager.config().value(), 56.0);
        assert_eq!(other_manager.config().name(), "test");
    }

    #[test]
    fn test_method_return_basic_type() {
        let methods = Methods::new();
        let id = methods.get_id();
        assert_eq!(id, 0);
    }

    #[test]
    fn test_method_arg_basic_type() {
        let mut methods = Methods::new();
        methods.set_id(123);
        let id = methods.id();
        assert_eq!(id, 123);
    }

    #[test]
    fn test_method_return_cpp_obj() {
        let methods = Methods::new();
        let config = methods.get_config();
        assert_eq!(config.id(), 42);
    }

    #[test]
    fn test_method_arg_cpp_obj() {
        let mut methods = Methods::new();
        let mut config = Config::new();
        config.set_id(100);
        methods.set_config(config.as_ref());
        assert_eq!(config.id(), 100);
    }
    #[test]
    fn test_method_arg_cpp_obj_by_value() {
        let mut methods = Methods::new();
        let mut config = Config::new();
        config.set_id(200);
        methods.set_config_by_value(config);
        let stored_config = methods.get_config();
        assert_eq!(stored_config.id(), 200);
    }

    #[test]
    fn test_static_method() {
        let result = Methods::add(10, 20);
        assert_eq!(result, 30);
    }

    #[test]
    fn test_method_return_option() {
        let mut methods = Methods::new();
        methods.set_id(100);
        let id_some = methods.optional_id(true);
        assert_eq!(id_some, Some(100));
        let id_none = methods.optional_id(false);
        assert_eq!(id_none, None);
    }

    #[test]
    fn test_method_return_obj_value() {
        let mut methods = Methods::new();
        let mut config = Config::new();
        config.set_id(120);
        methods.set_config(config.as_ref());
        let config = methods.create_config();
        assert_eq!(config.id(), 120);
    }

    #[test]
    fn test_iterator_method() {
        let mut container = ConfigContainer::new();
        let mut total_id = 0;
        for config in container.drain() {
            total_id += config.id();
        }
        assert_eq!(total_id, 300);
    }

    #[test]
    fn test_vec_obj() {
        let container = ConfigContainer::new();
        let vec = container.as_ref().data();
        assert_eq!(vec.len(), 2);
    }

    #[test]
    fn test_vec_obj_iter() {
        let mut container = ConfigContainer::new();
        let mut total_id = 0;
        for config in container.as_ref().data().iter() {
            total_id += config.id();
        }
        assert_eq!(total_id, 300);

        for mut config in container.data().iter_mut() {
            config.set_id(10);
        }
        for config in container.data().iter() {
            assert_eq!(config.id(), 10);
        }
    }

    #[test]
    fn test_vec_string() {
        let container = ConfigContainer::new();
        let vec = container.as_ref().names();
        assert_eq!(vec.len(), 2);
        for name in container.as_ref().names().iter() {
            assert!(name == "100" || name == "200");
        }
    }

    #[test]
    fn test_vec_number() {
        let mut container = ConfigContainer::new();
        let mut vec = container.ids();
        assert_eq!(vec.len(), 0);
        vec.push(10);
        vec.push(20);
        vec.push(30);
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.get(0).unwrap(), 10);
        let sum = vec.iter().fold(0, |a, b| a + b);
        assert_eq!(sum, 60);

        vec.iter_mut().for_each(|a| *a += 10);
        assert_eq!(vec.iter().fold(0, |a, b| a + b), 90);
    }

    #[test]
    fn test_option_val() {
        let mut chance = Chance::new();
        assert_eq!(chance.probability(), Some(75));
        chance.set_chance(false, 1);
        assert_eq!(chance.probability(), None);
    }

    #[test]
    fn test_option_obj() {
        let mut wallet = Wallet::new();
        assert_eq!(wallet.config().is_none(), true);

        let mut config = Config::new();
        config.set_id(555);
        wallet.set_config(true, config.as_ref());
        assert_eq!(wallet.config().is_some(), true);
        assert_eq!(wallet.config().unwrap().id(), 555);
    }

    #[test]
    fn test_map_obj() {
        let map_example = MapExample::new();
        let int_str_map = map_example.as_ref().int_str_map();
        assert_eq!(int_str_map.len(), 2);
        assert_eq!(int_str_map.get(1), Some("one".to_string()));
        assert_eq!(int_str_map.get(2), Some("two".to_string()));

        let int_config_map = map_example.as_ref().int_config_map();
        assert_eq!(int_config_map.len(), 2);
        assert_eq!(int_config_map.get(1), None);
        assert_eq!(int_config_map.get(10).unwrap().id(), 10);
    }

    #[test]
    fn test_map_key_string_val_obj() {
        let map_example = MapExample::new();
        let str_config_map = map_example.as_ref().str_config_map();
        assert_eq!(str_config_map.len(), 1);
        assert_eq!(str_config_map.get("one").unwrap().id(), 30);
    }
}
