FROM schickling/rust


RUN echo "deb http://httpredir.debian.org/debian wheezy main contrib\n\
deb http://httpredir.debian.org/debian wheezy-updates main\n\
deb http://security.debian.org wheezy/updates main" > /etc/apt/sources.list

RUN apt-get update

RUN apt-get install -y libsqlite3-dev

ADD . /src

RUN cd /src && cargo test

CMD [ "/bin/bash" ]
