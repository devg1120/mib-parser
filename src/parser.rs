use pest_consume::{ Parser, match_nodes, Error};
use regex::Regex;

#[derive(Parser)] // This allows Pest to add all the parse methods
#[grammar = "mib.pest"]
struct MibParser;

// Pull in the model
use crate::*;

// Some type simplifications, for brevity
type Result<T> = std::result::Result<T, Error<Rule>>;
type Node<'i> = pest_consume::Node<'i, Rule, ()>;
type Nodes<'i> = pest_consume::Nodes<'i, Rule, ()>;

pub fn parse_mib(mib_text: &str, options: &ParseOptions) -> Result<MibInfo> {
    let nodes = MibParser::parse(Rule::mib, mib_text)?;
    let main_node = nodes.single()?;

    if options.pretty_print {
        print_node(main_node.clone());
    }

    MibParser::mib(main_node)
}

// This is the other half of the parser, using pest_consume
// It traverses the Node tree generated by Pest (Nodes are a wrapper around Pest Pairs)
// and generates custom structures (MibInfo and friends) that represents the content of the MIB
#[pest_consume::parser]
impl MibParser {
    fn EOI(_node: Node) -> Result<()> {
        Ok(())
    }

    fn mib(node: Node) -> Result<MibInfo> {
        Ok(match_nodes!(node.into_children();
            [module_definition(mut defs).., EOI] => MibInfo{ modules: defs.collect()},
        ))
    }

    fn module_definition(node: Node) -> Result<Module> {
        Ok(match_nodes!(node.into_children();
            [module_identifier(mi), module_body(mbs)] => Module{ name: mi, assignments: mbs},
        ))
    }

    fn module_body(node: Node) -> Result<Vec<Assignment>> {
        Ok(match_nodes!(node.into_children();
            [assignment_list(a)] => a,
            [export_list(e), assignment_list(a)] => a,
            [import_list(i), assignment_list(a)] => a,
            [export_list(e), import_list(i), assignment_list(a)] => a,
        ))
    }

    fn import_list(node: Node) -> Result<String> {
        Ok(format!("{:?}", node.as_rule()))
    }

    fn export_list(node: Node) -> Result<String> {
        Ok(format!("{:?}", node.as_rule()))
    }

    fn assignment_list(node: Node) -> Result<Vec<Assignment>> {
        Ok(match_nodes!(node.into_children();
            [assignment(assignments)..] => assignments.collect(),
        ))
    }

    fn assignment(node: Node) -> Result<Assignment> {
        Ok(match_nodes!(node.into_children();
            [value_assignment(a)] => a,
            [type_assignment(a)] => a,
        ))
    }

    fn value_assignment(node: Node) -> Result<Assignment> {
        Ok(match_nodes!(node.into_children();
            [identifier(i), some_type(t), value(v)] => Assignment{name: i, a_type: t, value:Some(v)}
        ))
    }

    fn type_assignment(node: Node) -> Result<Assignment> {
        Ok(match_nodes!(node.into_children();
            [identifier(i), some_type(t)] => Assignment{name: i, a_type: t, value:None}
        ))
    }

    fn some_type(node: Node) -> Result<String> {
        Ok(format!("{:?}", node.as_rule()))
    }

    fn value(node: Node) -> Result<String> {
        Ok(node.as_str().to_owned())
    }

    fn module_identifier(node: Node) -> Result<String> {
        Ok(match_nodes!(node.into_children();
            [identifier(mi)] => mi.to_string(), // Without a value
            [identifier(mi), object_identifier_value(v)] => format!("{}={}", mi.to_string(), v), // With a value
        ))
    }

    fn object_identifier_value(node: Node) -> Result<String> {
        Ok(format!("{:?}", node.as_rule()))
    }

    fn identifier(node: Node) -> Result<String> {
        Ok(node.as_str().to_owned())
    }

    fn quoted_string(node: Node) -> Result<String> {
        Ok(match_nodes!(node.into_children();
            [inner_string(inner)] => inner,
        ))
    }

    fn inner_string(node: Node) -> Result<String> {
        let raw = node.as_str().to_owned();

        // Replace double quotes with single quotes
        let raw = raw.replace("\"\"", "\"");

        // Squelch newlines surrounded by spaces or tabs
        let re = Regex::new(r"[ \t]*\r?\n[ \t]*").unwrap();

        Ok(re.replace_all(raw.as_str(), "\n").to_string())
    }

    fn number_string(node: Node) -> Result<u64> {
        node.as_str().parse::<u64>().map_err(|e| node.error(e))
    }

    fn hex_string(node: Node) -> Result<u64> {
        let s = node.as_str();
        let len = s.len();
        // skip prefix and suffix
        u64::from_str_radix(&s[1..len-2], 16).map_err(|e| node.error(e))
    }

    fn binary_string(node: Node) -> Result<u64> {
        let s = node.as_str();
        let len = s.len();
        // skip prefix and suffix
        u64::from_str_radix(&s[1..len-2], 2).map_err(|e| node.error(e))
    }
}

//
// Helpers to print a readable parse tree, mainly for debug purposes
//

fn print_node(node: Node) {
    print_single_node(&node);
    print_nodes(node.children(), 1)
}

fn print_nodes(nodes: Nodes, level: usize) {
    for node in nodes {
        // A node is a combination of the rule which matched and a span of input
        print!("{:indent$}", "", indent=level*2);
        print_single_node(&node);

        // A node can be converted to an iterator of the tokens which make it up:
        print_nodes(node.children(), level+1);
    }
}

fn print_single_node(node: &Node) {
    match node.as_rule() {
        Rule::identifier => println!("{}", node.as_str()),
        Rule::number_string => println!("{}", node.as_str()),
        Rule::inner_string => println!("{}", clean_string(node.as_str())),
        _ => println!("<<{:?}>>", node.as_rule())
    }
}

fn clean_string(s: &str) -> String {
        // Squelch newlines surrounded by spaces or tabs
        let re = Regex::new(r"[ \t]*\r?\n[ \t]*").unwrap();
        format!( "\"{}\"", re.replace_all(s, "\\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number() {
        let number = MibParser::number_string(parse(Rule::number_string, "12345678")).unwrap();
        assert_eq!(number, 12345678);
    }

    #[test]
    fn not_a_number() {
        parse_fail(Rule::number_string, "A1234");
    }

    #[test]
    fn quoted_string_1() {
        let result = MibParser::quoted_string(parse(Rule::quoted_string, r#""this is a quoted string""#)).unwrap();
        assert_eq!(result, "this is a quoted string");
    }

    #[test]
    fn quoted_string_2() {
        let result = MibParser::quoted_string(parse(Rule::quoted_string, r#""this is a ""quoted"" string""#)).unwrap();
        assert_eq!(result, r#"this is a "quoted" string"#);
    }

    #[test]
    fn quoted_string_3() {
        let result = MibParser::quoted_string(parse(Rule::quoted_string, "\"this is a    \n   quoted string\"")).unwrap();
        assert_eq!(result, "this is a\nquoted string");

        let result = MibParser::quoted_string(parse(Rule::quoted_string, "\"this is a    \r\n   quoted string\"")).unwrap();
        assert_eq!(result, "this is a\nquoted string");
    }

    #[test]
    fn binary_string() {
        let result = MibParser::binary_string(parse(Rule::binary_string, "'11110000'b")).unwrap();
        assert_eq!(result, 0b11110000);
    }

    #[test]
    fn hex_string() {
        let result = MibParser::hex_string(parse(Rule::hex_string, "'DEADBEEF'H")).unwrap();
        assert_eq!(result, 0xDEADBEEF);
    }

    #[test]
    fn identifier_0() {
        let identifier = MibParser::identifier(parse(Rule::identifier, "ab1ur_d-gh0")).unwrap();
        assert_eq!(identifier, "ab1ur_d-gh0");
    }

    #[test]
    fn identifier_1() {
        parse_fail(Rule::identifier, "0abc");
    }

    #[test]
    fn identifier_2() {
        parse_fail(Rule::identifier, "_abc");
    }

    #[test]
    fn object_id_0() {
        let node = parse(Rule::value_assignment, "synology OBJECT IDENTIFIER ::= { enterprises 6574 }");
        print_node(node)
    }

    #[test]
    fn sequence_1() {
        let node = parse(Rule::sequence_of_type, "SEQUENCE OF wibble");
        print_node(node)
    }

    #[test]
    fn snmp_update() {
        let input = r#"LAST-UPDATED "201309110000Z""#;
        let node = parse(Rule::snmp_update_part, input);
        print_node(node)
    }

    #[test]
    fn some_type() {
        let input = r#"MODULE-IDENTITY
        LAST-UPDATED "201309110000Z"
        ORGANIZATION "www.synology.com"
        CONTACT-INFO
             "postal:   Jay Pan
              email:    jaypan@synology.com"
        DESCRIPTION
            "Characteristics of the disk information"
        REVISION     "201309110000Z"
        DESCRIPTION
            "Second draft.""#;

        let node = parse(Rule::some_type, input);
        print_node(node)
    }

    #[test]
    fn value() {
        let input = "{ synology 2 }";

        let node = parse(Rule::value, input);
        print_node(node)
    }

    #[test]
    fn constraint_list() {
        let input = "( SIZE (0..63) )";
        let node = parse(Rule::constraint_list, input);
        print_node(node)        
    }

    #[test]
    fn value_assignment() {
        let input = r#"synoDisk MODULE-IDENTITY
            LAST-UPDATED "201309110000Z"
            ORGANIZATION "www.synology.com"
            CONTACT-INFO
                 "postal:   Jay Pan
                  email:    jaypan@synology.com"
            DESCRIPTION
                "Characteristics of the disk information"
            REVISION     "201309110000Z"
            DESCRIPTION
                "Second draft."
            ::= { synology 2 }"#;

        let node = parse(Rule::value_assignment, input);
        print_node(node)        
    }

    #[test]
    fn import_list() {
        let input = r#"IMPORTS
        OBJECT-GROUP, MODULE-COMPLIANCE
                    FROM SNMPv2-CONF
        enterprises, MODULE-IDENTITY, OBJECT-TYPE, Integer32
                    FROM SNMPv2-SMI;"#;

        let node = parse(Rule::import_list, input);
        print_node(node)
    }

    #[test]
    fn assignment() {
        let input = r#"synoDisk MODULE-IDENTITY
            LAST-UPDATED "201309110000Z"
            ORGANIZATION "www.synology.com"
            CONTACT-INFO
                 "postal:   Jay Pan
                  email:    jaypan@synology.com"
            DESCRIPTION
                "Characteristics of the disk information"
            REVISION     "201309110000Z"
            DESCRIPTION
                "Second draft."
            ::= { synology 2 }"#;

        let node = parse(Rule::value_assignment, input);
        print_node(node)        
    }

    #[test]
    fn module_body() {
        let input = r#"IMPORTS
            OBJECT-GROUP, MODULE-COMPLIANCE
                        FROM SNMPv2-CONF
            enterprises, MODULE-IDENTITY, OBJECT-TYPE, Integer32
                        FROM SNMPv2-SMI;
        
        synoDisk MODULE-IDENTITY
            LAST-UPDATED "201309110000Z"
            ORGANIZATION "www.synology.com"
            CONTACT-INFO
                 "postal:   Jay Pan
                  email:    jaypan@synology.com"
            DESCRIPTION
                "Characteristics of the disk information"
            REVISION     "201309110000Z"
            DESCRIPTION
                "Second draft."
            ::= { synology 2 }"#;

        let node = parse(Rule::module_body, input);
        print_node(node)
    }

    #[test]
    fn value_test1() {
        // A very simple value, for example, used in groups
        let input = "rmonEtherStatsGroup";
        let node = parse(Rule::value, input);
        print_node(node)
    }

    #[test]
    fn compliance_group() {
        let input = r#"GROUP rmonEtherStatsGroup
        DESCRIPTION
            "The RMON Ethernet Statistics Group is optional.""#;
        let node = parse(Rule::compliance_group, input);
        print_node(node)
    }

    #[test]
    fn snmp_module_part() {
        let input = r#"MODULE -- this module       
              GROUP rmonEtherStatsGroup
                  DESCRIPTION
                      "The RMON Ethernet Statistics Group is optional.""#;
        let node = parse(Rule::snmp_module_part, input);
        print_node(node)
    }

    //
    // test helpers
    //
    fn parse(rule: Rule, input: &str) -> Node {
        match MibParser::parse(rule, input) {
            Ok(nodes) => {
                let node = nodes.single().unwrap();
                assert_eq!(node.as_rule(), rule);
                if node.as_str() != input {
                    println!("Expected rule({:?}) to fully consume '{}'", rule, input);
                    print_node(node);
                    panic!("Failed test");
                }
                node 
            },
            Err(e) => panic!("Parse failed: {}", e)
        }
    }

    fn parse_fail(rule: Rule, input: &str) {
        assert!(MibParser::parse(rule, input).is_err(), "Expected rule({:?}) to fail to parse '{}'", rule, input);
    }
}