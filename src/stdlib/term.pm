MOD term:
    export tty_available, screen_size, screen_cols, screen_rows
    export screen_begin, screen_end, move_to, write_text, write_at
    export clear_screen_view, clear_row, clear_to_end_of_line
    export poll_key, wait_key, show_cursor_now, hide_cursor_now, flush_output, draw_frame, draw_buffer_view, edit_line, edit_buffer, edit_buffer_safe

DEF tty_available():
    RET.NOW(): term_is_tty()
END

DEF screen_size():
    RET.NOW(): term_size()
END

DEF screen_cols():
    dims = term_size()
    RET.NOW(): dims[0]
END

DEF screen_rows():
    dims = term_size()
    RET.NOW(): dims[1]
END

DEF screen_begin():
    term_raw_enable()
    term_enter_alt_screen()
    term_hide_cursor()
    term_clear_screen()
    term_move_cursor(1, 1)
    term_flush()
END

DEF screen_end():
    term_show_cursor()
    term_leave_alt_screen()
    term_raw_disable()
    term_flush()
END

DEF move_to(row, col):
    term_move_cursor(row, col)
END

DEF write_text(text):
    term_write(text)
END

DEF write_at(row, col, text):
    term_move_cursor(row, col)
    term_write(text)
END

DEF clear_screen_view():
    term_clear_screen()
END

DEF clear_row(row):
    term_move_cursor(row, 1)
    term_clear_line()
END

DEF clear_to_end_of_line():
    term_clear_to_end()
END

DEF poll_key():
    RET.NOW(): term_read_key(0)
END

DEF wait_key(timeout_ms):
    RET.NOW(): term_read_key(timeout_ms)
END

DEF show_cursor_now():
    term_show_cursor()
END

DEF hide_cursor_now():
    term_hide_cursor()
END

DEF flush_output():
    term_flush()
END

DEF draw_frame(rows, cursor_row, cursor_col):
    term_draw_frame(rows, cursor_row, cursor_col)
END

DEF draw_buffer_view(lines, scroll_row, scroll_col, gutter, status_line, cursor_row, cursor_col):
    term_draw_buffer_view(lines, scroll_row, scroll_col, gutter, status_line, cursor_row, cursor_col)
END

DEF edit_line(prompt_text, initial_text):
    RET.NOW(): TERM_EDIT_LINE(prompt_text, initial_text)
END

DEF edit_buffer(lines, status_line, row, col):
    RET.NOW(): TERM_EDIT_BUFFER(lines, status_line, row, col)
END

DEF edit_buffer_safe(lines, status_line, row, col, timeout_ms):
    RET.NOW(): TERM_EDIT_BUFFER(lines, status_line, row, col, timeout_ms)
END

END
