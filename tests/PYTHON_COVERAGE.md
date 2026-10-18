# Python test coverage

Every original test function has a Rust equivalent. Parameter sets are retained in the corresponding loops. DNS is injected in Rust tests, avoiding public DNS. Dynamic non-string spreadsheet values cannot enter the typed Rust row API; this is documented in README.

| Original Python test | Rust equivalent |
|---|---|
| `test_feeds.py::test_first_text_returns_the_first_tag_that_has_a_value` | `tests/feeds.rs::first_text_returns_the_first_tag_that_has_a_value` |
| `test_feeds.py::test_first_text_prefers_the_earlier_tag` | `tests/feeds.rs::first_text_prefers_the_earlier_tag` |
| `test_feeds.py::test_first_text_skips_a_tag_that_is_present_but_empty` | `tests/feeds.rs::first_text_skips_a_tag_that_is_present_but_empty` |
| `test_feeds.py::test_first_text_falls_back_to_the_default` | `tests/feeds.rs::first_text_falls_back_to_the_default` |
| `test_feeds.py::test_first_text_does_not_discard_an_element_without_children` | `tests/feeds.rs::first_text_does_not_discard_an_element_without_children` |
| `test_feeds.py::test_a_universal_feed_yields_every_image` | `tests/feeds.rs::a_universal_feed_yields_every_image` |
| `test_feeds.py::test_a_google_feed_yields_every_image` | `tests/feeds.rs::a_google_feed_yields_every_image` |
| `test_feeds.py::test_a_heureka_feed_yields_every_image` | `tests/feeds.rs::a_heureka_feed_yields_every_image` |
| `test_feeds.py::test_the_three_feed_shapes_agree_on_what_they_return` | `tests/feeds.rs::the_three_feed_shapes_agree_on_what_they_return` |
| `test_feeds.py::test_a_product_without_an_image_is_left_out` | `tests/feeds.rs::a_product_without_an_image_is_left_out` |
| `test_feeds.py::test_a_missing_product_number_gets_a_generated_one` | `tests/feeds.rs::a_missing_product_number_gets_a_generated_one` |
| `test_feeds.py::test_an_empty_feed_yields_nothing` | `tests/feeds.rs::an_empty_feed_yields_nothing` |
| `test_feeds.py::test_the_index_counts_products_from_one` | `tests/feeds.rs::the_index_counts_products_from_one` |
| `test_feeds.py::test_a_product_row_follows_the_header_order` | `tests/feeds.rs::a_product_row_follows_the_header_order` |
| `test_feeds.py::test_missing_product_fields_become_empty_strings` | `tests/feeds.rs::missing_product_fields_become_empty_strings` |
| `test_feeds.py::test_every_feed_type_declares_the_same_columns` | `tests/feeds.rs::every_feed_type_declares_the_same_columns` |
| `test_feeds.py::test_every_feed_type_has_a_path_and_a_name` | `tests/feeds.rs::every_feed_type_has_a_path_and_a_name` |
| `test_feeds.py::test_a_parser_exists_for_every_feed_type` | `tests/feeds.rs::a_parser_exists_for_every_feed_type` |
| `test_feeds.py::test_an_unknown_feed_type_is_read_as_a_universal_feed` | `tests/feeds.rs::an_unknown_feed_type_is_read_as_a_universal_feed` |
| `test_feeds.py::test_a_feed_file_is_parsed` | `tests/feeds.rs::a_feed_file_is_parsed` |
| `test_feeds.py::test_a_malformed_feed_returns_none` | `tests/feeds.rs::a_malformed_feed_returns_none` |
| `test_feeds.py::test_a_feed_declaring_entities_is_refused` | `tests/feeds.rs::a_feed_declaring_entities_is_refused` |
| `test_security.py::test_an_address_on_this_machine_or_network_is_refused` | `tests/security.rs::an_address_on_this_machine_or_network_is_refused` |
| `test_security.py::test_only_http_and_https_are_allowed` | `tests/security.rs::only_http_and_https_are_allowed` |
| `test_security.py::test_a_url_without_a_host_is_refused` | `tests/security.rs::a_url_without_a_host_is_refused` |
| `test_security.py::test_a_hostname_that_does_not_resolve_is_refused` | `tests/security.rs::a_hostname_that_does_not_resolve_is_refused` |
| `test_security.py::test_a_public_address_is_allowed` | `tests/security.rs::a_public_address_is_allowed` |
| `test_security.py::test_a_value_a_spreadsheet_would_run_is_quoted` | `tests/security.rs::a_value_a_spreadsheet_would_run_is_quoted` |
| `test_security.py::test_a_plain_product_name_is_left_alone` | `tests/security.rs::a_plain_product_name_is_left_alone` |
| `test_security.py::test_a_value_that_is_not_text_passes_through` | `tests/security.rs::a_value_that_is_not_text_passes_through` |
| `test_security.py::test_a_negative_number_written_as_text_is_quoted` | `tests/security.rs::a_negative_number_written_as_text_is_quoted` |
| `test_security.py::test_a_character_windows_forbids_is_replaced` | `tests/security.rs::a_character_windows_forbids_is_replaced` |
| `test_security.py::test_a_path_cannot_be_smuggled_through_a_product_name` | `tests/security.rs::a_path_cannot_be_smuggled_through_a_product_name` |
| `test_security.py::test_czech_characters_survive` | `tests/security.rs::czech_characters_survive` |
| `test_security.py::test_a_plain_feed_parses` | `tests/security.rs::a_plain_feed_parses` |
| `test_security.py::test_an_entity_expansion_payload_is_refused` | `tests/security.rs::an_entity_expansion_payload_is_refused` |
| `test_security.py::test_an_external_entity_is_refused` | `tests/security.rs::an_external_entity_is_refused` |
| `test_security.py::test_an_unsafe_url_is_not_fetched` | `tests/security.rs::an_unsafe_url_is_not_fetched` |
| `test_security.py::test_a_declared_size_over_the_limit_is_refused` | `tests/security.rs::a_declared_size_over_the_limit_is_refused` |
| `test_security.py::test_a_server_that_lies_about_the_size_is_still_stopped` | `tests/security.rs::a_server_that_lies_about_the_size_is_still_stopped` |
| `test_security.py::test_a_normal_image_comes_back_whole` | `tests/security.rs::a_normal_image_comes_back_whole` |
| `test_security.py::test_a_nonsense_content_length_header_does_not_crash` | `tests/security.rs::a_nonsense_content_length_header_does_not_crash` |
| `test_security.py::test_the_peer_address_is_read_off_the_socket` | `tests/network.rs::the_peer_address_is_read_off_the_socket` |
| `test_security.py::test_a_response_without_a_socket_reports_no_peer` | `tests/security.rs::a_response_without_a_socket_reports_no_peer` |
| `test_security.py::test_a_connection_that_landed_on_a_private_address_is_dropped` | `tests/network.rs::a_connection_that_landed_on_a_private_address_is_dropped` |
| `test_security.py::test_every_private_peer_is_refused_after_connecting` | `tests/security.rs::every_private_peer_is_refused_after_connecting` |
| `test_security.py::test_a_public_peer_is_read_normally` | `tests/security.rs::a_public_peer_is_read_normally` |
