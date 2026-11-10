use crate::schema::{Field, Schema, Shape};
use alloc::{string::String, vec::Vec};
use berm_abi::events::{Event, Page};

impl<T: Schema + 'static> Schema for Event<T> {
    const SHAPE: Shape = Shape::Object {
        title: "",
        fields: &[
            Field {
                name: "id",
                description: "",
                shape: &String::SHAPE,
            },
            Field {
                name: "cursor",
                description: "",
                shape: &String::SHAPE,
            },
            Field {
                name: "at",
                description: "",
                shape: &u64::SHAPE,
            },
            Field {
                name: "data",
                description: "",
                shape: &T::SHAPE,
            },
        ],
    };
}

impl<T: Schema + 'static> Schema for Page<T> {
    const SHAPE: Shape = Shape::Object {
        title: "",
        fields: &[
            Field {
                name: "events",
                description: "",
                shape: &Vec::<Event<T>>::SHAPE,
            },
            Field {
                name: "cursor",
                description: "",
                shape: &String::SHAPE,
            },
            Field {
                name: "has_more",
                description: "",
                shape: &bool::SHAPE,
            },
        ],
    };
}
