pub fn main() {
    let statement = "CREATE VIEW MyView AS SELECT a, b FROM Table1 WHERE a = True;";
    let isl_catalog = get_isl_catalog();
    let catalog = Catalog::from_isl(isl_catalog);

    assert_eq!(catalog, Catalog {
        schemata: vec![
            schema: Schema {
                schema_type: SchemaType::View,
                attrs: [
                    Attr {
                        attr_name: "a",
                        ty: Type::PARTIQL_INT8()
                    },
                    Attr {
                        attr_name: "b",
                        ty: Type::PARTIQL_VARCHAR
                    }
                ],
            }
        ],
        predicate: Predicate {
            condition: {
                ty: BinOp::Eq,
                lhs: Attr {
                    attr_name: "a",
                    ty: PARTIQL_INT8
                },
                rhs: Value::PARTIQL_INT8(2)
            }
        }
    });

    let Ok(view_schema) = infer(statement, catalog);

    let isl = schema.to_isl();
}