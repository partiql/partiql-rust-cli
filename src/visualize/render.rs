use crate::visualize::ast_to_dot::AstToDot;

use std::convert::AsRef;
use std::io::Write;
use std::os::raw::c_char;
use std::slice;
use strum::AsRefStr;

use crate::visualize::common::ToDotGraph;
use crate::visualize::plan_to_dot::PlanToDot;
use graphviz_sys as gv;
use partiql_ast::ast;
use partiql_ast::ast::{AstNode, Expr, TopLevelQuery};
use partiql_logical::{BindingsOp, LogicalPlan};
use serde::Serialize;
use tiny_skia::Transform;
use usvg::TreeParsing;

/// Convert an AST into JSON
#[inline]
pub fn to_json<T>(data: &T) -> String
where
    T: ?Sized + Serialize,
{
    serde_json::to_string_pretty(&data).expect("json print")
}

/// Graphviz output formats
#[derive(AsRefStr, Debug, Copy, Clone)]
#[strum(serialize_all = "lowercase")]
#[non_exhaustive]
pub enum GraphVizFormat {
    /// Pretty-print
    Canon,
    /// Pretty-print; internal alias for graphviz's `canon`
    /// #[strum(serialize = "cannon")]
    PrettyPrint,
    /// Attributed dot
    Dot,
    /// Extended dot
    XDot,
    /// Svg
    Svg,
    /// Png
    Png,
}

/// FFI to graphviz-sys to convert a dot-formatted graph into the specified format.
fn gv_render(format: GraphVizFormat, graph_str: String) -> Vec<u8> {
    let c_graph_str = std::ffi::CString::new(graph_str).expect("cstring new failed");
    let c_dot = std::ffi::CString::new("dot").expect("cstring new failed");
    let c_format = std::ffi::CString::new(format.as_ref()).expect("cstring new failed");

    unsafe {
        let gvc = gv::gvContext();
        // TODO gvParseArgs to pass 'theme' colors, etc?
        //    See section 4 of https://www.graphviz.org/pdf/libguide.pdf
        //    See `dot --help`
        let g = gv::agmemread(c_graph_str.as_ptr());

        gv::gvLayout(gvc, g, c_dot.as_ptr());

        let mut buffer_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
        let mut length = 0 as std::os::raw::c_uint;
        gv::gvRenderData(gvc, g, c_format.as_ptr(), &mut buffer_ptr, &mut length);
        let c_bytes = slice::from_raw_parts_mut(buffer_ptr, length as usize);

        let bytes = std::mem::transmute::<&mut [c_char], &[u8]>(c_bytes);
        let out = Vec::from(bytes);

        gv::gvFreeRenderData(buffer_ptr);
        gv::gvFreeLayout(gvc, g);
        gv::agclose(g);
        gv::gvFreeContext(gvc);

        out
    }
}

pub struct Graph(pub String);

impl Into<Graph> for &AstNode<TopLevelQuery> {
    fn into(self) -> Graph {
        Graph(AstToDot::default().to_graph(self))
    }
}

impl Into<Graph> for &Box<ast::Expr> {
    fn into(self) -> Graph {
        Graph(AstToDot::default().to_graph(self))
    }
}

impl Into<Graph> for &LogicalPlan<BindingsOp> {
    fn into(self) -> Graph {
        Graph(PlanToDot::default().to_graph(self))
    }
}

/// FFI to graphviz-sys to convert a dot-formatted graph into the specified text format.
#[inline]
fn render_to_string<T>(format: GraphVizFormat, data: T) -> String
where
    T: Into<Graph>,
{
    let Graph(graph_str) = data.into();
    String::from_utf8(gv_render(format, graph_str)).expect("valid utf8")
}

/// Convert an AST into an attributed dot graph.
#[inline]
pub fn to_dot<T>(data: T) -> String
where
    T: Into<Graph>,
{
    render_to_string(GraphVizFormat::Dot, data)
}

/// Convert an AST into a pretty-printed dot graph.
#[inline]
pub fn to_pretty_dot<T>(data: T) -> String
where
    T: Into<Graph>,
{
    render_to_string(GraphVizFormat::Canon, data)
}

/// Convert an AST into a graphviz svg.
#[inline]
pub fn to_svg<T>(data: T) -> String
where
    T: Into<Graph>,
{
    render_to_string(GraphVizFormat::Svg, data)
}

/// Convert an AST into a graphviz svg and render it to png.
pub fn to_png<T>(data: T) -> Vec<u8>
where
    T: Into<Graph>,
{
    let svg_data = to_svg(data);

    let mut opt = usvg::Options::default();

    let rtree = usvg::Tree::from_data(svg_data.as_bytes(), &opt).unwrap();
    let pixmap_size = rtree.size.to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();
    let rtree = resvg::Tree::from_usvg(&rtree);
    rtree.render(Transform::default(), &mut pixmap.as_mut());
    pixmap.encode_png().expect("png encoding failed")
}

/// Convert an AST into a graphviz svg and render it to png, then display in the console.
pub fn display<T>(data: T)
where
    T: Into<Graph>,
{
    let png = to_png(data);

    let conf = viuer::Config {
        absolute_offset: false,
        transparent: true,
        use_sixel: false,
        ..Default::default()
    };

    let img = image::load_from_memory(&png).expect("png loading failed.");
    viuer::print(&img, &conf).expect("Image printing failed.");
}
