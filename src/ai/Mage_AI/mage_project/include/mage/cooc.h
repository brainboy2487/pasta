#ifndef MAGE_COOC_H
#define MAGE_COOC_H

#include "types.h"

cooc_table_t* cooc_table_create(size_t initial_capacity);
void cooc_table_destroy(cooc_table_t* table);
int cooc_table_insert(cooc_table_t* table, uint32_t from, uint32_t to, uint8_t ctx);
uint32_t cooc_table_get(const cooc_table_t* table, uint32_t from, uint32_t to, uint8_t ctx);

#endif
