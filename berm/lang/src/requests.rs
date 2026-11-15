use crate::schema::{Field, Schema, Shape};
use alloc::string::String;
use berm_abi::requests::{Execution, Review};

impl<T: Schema + 'static> Schema for Review<T> {
    const SHAPE: Shape = Shape::Object {
        title: "",
        fields: &[
            Field {
                name: "title",
                description: "",
                shape: &String::SHAPE,
            },
            Field {
                name: "body",
                description: "",
                shape: &String::SHAPE,
            },
            Field {
                name: "tool",
                description: "",
                shape: &String::SHAPE,
            },
            Field {
                name: "input",
                description: "",
                shape: &T::SHAPE,
            },
        ],
    };
}

impl<T: Schema + 'static> Schema for Execution<T> {
    const SHAPE: Shape = Shape::Object {
        title: "",
        fields: &[
            Field {
                name: "request",
                description: "",
                shape: &String::SHAPE,
            },
            Field {
                name: "input",
                description: "",
                shape: &T::SHAPE,
            },
        ],
    };
}
