import { InMemoryPlacesRepository, sharedRepositoryContractTests } from "@webglobe/core";
import { RemotePlacesRepository } from "../src/RemotePlacesRepository.js";

sharedRepositoryContractTests(() => new RemotePlacesRepository(new InMemoryPlacesRepository()));
