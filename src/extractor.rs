use ego_tree::iter::Edge;
use ego_tree::NodeRef;
use regex::Regex;
use scraper::Node;
use scraper::{ElementRef, Html};

use crate::types::doc_node::DocNode;

/**
 * We apply special treatment to <hr/> tags found in the raw HTML.
 * The parser forces all open tags closed as soon as we discover an hr.
 * We want to leave these open so that we can try and retain our nested node structured
 * as best as possible.
 *
 * We temporarily replace these with the hrbr tag to avoid this parser behavior and swap these back after
 * parsing.
 */
static HRBR_PLACEHOLDER: &str = "hrbr";

pub fn esc_hr(hrstr: String) -> String {
    let re = Regex::new(r"</?hr/?>").unwrap();
    return re
        .replace_all(&hrstr, format!("<{HRBR_PLACEHOLDER}></{HRBR_PLACEHOLDER}>"))
        .to_string();
}

pub fn has_text_node(node: NodeRef<Node>) -> bool {
    node.children().any(|child_node| {
        match child_node.value() {
            Node::Element(element) => {
                // Recursively check children like <br> or other elements that might contain text
                 element.name() == "br" || has_text_node(child_node)
            }
            Node::Text(text_node) => {
                // Check based on context (inside <pre> or not)
                if is_inside_pre(child_node) {
                    !text_node.text.is_empty() // Keep if not completely empty inside <pre>
                } else {
                    !text_node.text.trim().is_empty() // Keep if not just whitespace outside <pre>
                }
            }
            _ => false, // Ignore comments, doctypes etc.
        }
    })
}

// Helper function to check if a node is inside a <pre> element
fn is_inside_pre(node: NodeRef<Node>) -> bool {
    node.ancestors().any(|ancestor| {
        if let Some(element) = ancestor.value().as_element() {
            element.name() == "pre"
        } else {
            false
        }
    })
}

/**
 * We parse a raw scraper::HTML and return a
 * list of leaf doc nodes  (each with a linked list pointer to the root)
 * for us to attempt to transform into an ADF Document
 */
pub fn extract_leaves(fragment: &Html) -> Vec<DocNode> {
    let mut leaf_nodes: Vec<DocNode> = Vec::new();
    fragment
        .root_element()
        .traverse()
        .for_each(|edge| match edge {
            Edge::Close(node) => {
                if let Some(element) = ElementRef::wrap(node) {
                    let name = element.value().name();
                    // Handle self-closing or special leaf nodes
                    if name == "iframe" || name == "img" {
                        leaf_nodes.push(DocNode {
                            name: name.trim(), // Use the actual name
                            text: "".to_owned(), // No text content for these
                            node,
                        })
                    } else if name == HRBR_PLACEHOLDER {
                        leaf_nodes.push(DocNode {
                            name: "hr", // Restore original name
                            text: "".to_owned(),
                            node,
                        })
                    } else if name == "br" {
                         leaf_nodes.push(DocNode {
                            name: "br",
                            text: "".to_owned(),
                            node,
                        })
                    } else if name == "td" {
                        // Add TD node only if it's genuinely empty (doesn't contain significant text nodes)
                        if !has_text_node(node) {
                            leaf_nodes.push(DocNode {
                                name: "td",
                                text: "".to_owned(),
                                node,
                            })
                        }
                    }
                    // Other closing tags like </font>, </p>, </li> etc. are handled implicitly
                    // by the traversal and the text node logic below.

                } else if let Node::Text(text_node) = node.value() {
                    // Only consider text nodes that have a parent element
                    if node.parent().is_some() {
                        let text_content = &text_node.text;
                        let inside_pre = is_inside_pre(node);

                        // Determine if this text node should be kept
                        let should_keep = if inside_pre {
                            // Inside <pre>: Keep if it's not completely empty. Preserve all whitespace.
                            !text_content.is_empty()
                        } else {
                            // Outside <pre>: Keep only if it contains non-whitespace characters.
                            !text_content.trim().is_empty()
                        };

                        if should_keep {
                            leaf_nodes.push(DocNode {
                                name: "text",
                                // IMPORTANT: Always store the *original* text content.
                                text: text_content.to_string(),
                                node,
                            })
                        }
                    }
                }
            }
            Edge::Open(_) => (),
        });
    leaf_nodes
}
