CC = gcc
CFLAGS = -O2 -Wall -Wextra -std=c99

TARGET = mage_markov
SRC = text_tokenizer.c

all: $(TARGET)

$(TARGET): $(SRC)
	$(CC) $(CFLAGS) -o $(TARGET) $(SRC)

clean:
	rm -f $(TARGET)
	rm -f M*_data.txt
